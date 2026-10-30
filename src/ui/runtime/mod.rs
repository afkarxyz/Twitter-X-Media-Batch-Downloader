//! UI-thread state, live row models and a worker-to-egui event queue.
use std::{
    any::Any,
    cell::{Cell, RefCell},
    collections::{HashMap, VecDeque},
    marker::PhantomData,
    rc::Rc,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

pub mod state;
use state::*;
pub type SharedString = String;
pub type Color = egui::Color32;

#[derive(Debug)]
pub enum PlatformError {
    Other(String),
}
impl std::fmt::Display for PlatformError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Other(s) => f.write_str(s),
        }
    }
}
impl std::error::Error for PlatformError {}
pub type EventLoopError = PlatformError;

type Event = Box<dyn FnOnce() + Send>;
static EVENTS: Mutex<VecDeque<Event>> = Mutex::new(VecDeque::new());
static WAKER: OnceLock<fastframe_shell::Waker> = OnceLock::new();
static NEXT_ID: AtomicU64 = AtomicU64::new(1);
static QUIT: AtomicBool = AtomicBool::new(false);
thread_local! {
    static WINDOWS: RefCell<HashMap<u64, std::rc::Weak<RefCell<State>>>> = RefCell::default();
    static TIMERS: RefCell<Vec<Scheduled>> = RefCell::default();
}
pub fn invoke_from_event_loop(event: impl FnOnce() + Send + 'static) -> Result<(), EventLoopError> {
    EVENTS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push_back(Box::new(event));
    if let Some(waker) = WAKER.get() {
        waker.wake();
    }
    Ok(())
}
pub fn quit_event_loop() -> Result<(), EventLoopError> {
    QUIT.store(true, Ordering::Relaxed);
    if let Some(waker) = WAKER.get() {
        waker.wake();
    }
    Ok(())
}
pub fn attach(ctx: &egui::Context) {
    WAKER.get_or_init(Default::default).attach(ctx);
}
pub fn pump(ctx: &egui::Context) {
    let events: Vec<_> = EVENTS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .drain(..)
        .collect();
    for event in events {
        event();
    }
    let now = Instant::now();
    let mut ready = Vec::new();
    TIMERS.with_borrow_mut(|timers| {
        let mut i = 0;
        while i < timers.len() {
            if timers[i].cancel.load(Ordering::Relaxed) {
                timers.remove(i);
            } else if timers[i].deadline <= now {
                ready.push(timers.remove(i));
            } else {
                i += 1;
            }
        }
    });
    for mut timer in ready {
        if !timer.cancel.load(Ordering::Relaxed) {
            (timer.callback)();
        }
        if let Some(interval) = timer.interval {
            timer.deadline = Instant::now() + interval;
            TIMERS.with_borrow_mut(|timers| timers.push(timer));
        }
    }
    TIMERS.with_borrow(|timers| {
        if let Some(deadline) = timers.iter().map(|t| t.deadline).min() {
            ctx.request_repaint_after(deadline.saturating_duration_since(Instant::now()));
        }
    });
    if QUIT.load(Ordering::Relaxed) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
}
struct Scheduled {
    deadline: Instant,
    interval: Option<Duration>,
    callback: Box<dyn FnMut()>,
    cancel: Arc<AtomicBool>,
}
#[derive(Default)]
pub struct Timer {
    cancel: RefCell<Option<Arc<AtomicBool>>>,
}
pub enum TimerMode {
    Repeated,
}
impl Timer {
    pub fn start(&self, mode: TimerMode, duration: Duration, callback: impl FnMut() + 'static) {
        self.stop();
        let cancel = Arc::new(AtomicBool::new(false));
        *self.cancel.borrow_mut() = Some(cancel.clone());
        TIMERS.with_borrow_mut(|timers| {
            timers.push(Scheduled {
                deadline: Instant::now() + duration,
                interval: matches!(mode, TimerMode::Repeated).then_some(duration),
                callback: Box::new(callback),
                cancel,
            })
        });
    }
    pub fn stop(&self) {
        if let Some(cancel) = self.cancel.borrow_mut().take() {
            cancel.store(true, Ordering::Relaxed);
        }
    }
    pub fn single_shot(duration: Duration, callback: impl FnOnce() + 'static) {
        let mut callback = Some(callback);
        TIMERS.with_borrow_mut(|timers| {
            timers.push(Scheduled {
                deadline: Instant::now() + duration,
                interval: None,
                callback: Box::new(move || {
                    if let Some(f) = callback.take() {
                        f();
                    }
                }),
                cancel: Arc::new(AtomicBool::new(false)),
            })
        });
    }
}
impl Drop for Timer {
    fn drop(&mut self) {
        self.stop();
    }
}

#[derive(Clone)]
pub struct AppWindow {
    pub(super) state: Rc<RefCell<State>>,
    id: u64,
}
impl AppWindow {
    pub fn new() -> Result<Self, PlatformError> {
        let app = Self {
            state: Rc::new(RefCell::new(State::default())),
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
        };
        WINDOWS.with_borrow_mut(|windows| {
            windows.retain(|_, window| window.strong_count() > 0);
            windows.insert(app.id, Rc::downgrade(&app.state));
        });
        Ok(app)
    }
    pub fn as_weak(&self) -> Weak<Self> {
        Weak {
            id: self.id,
            marker: PhantomData,
        }
    }
    pub fn global<G: Global>(&self) -> G {
        G::from_app(self)
    }
    pub fn run(&self) -> Result<(), PlatformError> {
        crate::ui::run(self.clone())
    }
}
pub trait Global {
    fn from_app(app: &AppWindow) -> Self;
}
pub struct Weak<T> {
    id: u64,
    marker: PhantomData<fn() -> T>,
}
impl<T> Clone for Weak<T> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            marker: PhantomData,
        }
    }
}
impl<T> Default for Weak<T> {
    fn default() -> Self {
        Self {
            id: 0,
            marker: PhantomData,
        }
    }
}
impl Weak<AppWindow> {
    pub fn upgrade(&self) -> Option<AppWindow> {
        WINDOWS.with_borrow(|windows| {
            windows
                .get(&self.id)
                .and_then(std::rc::Weak::upgrade)
                .map(|state| AppWindow { state, id: self.id })
        })
    }
    pub fn upgrade_in_event_loop(
        &self,
        event: impl FnOnce(AppWindow) + Send + 'static,
    ) -> Result<(), EventLoopError> {
        let weak = self.clone();
        invoke_from_event_loop(move || {
            if let Some(app) = weak.upgrade() {
                event(app);
            }
        })
    }
}

pub trait Model: Any {
    type Data: Clone + 'static;
    fn row_count(&self) -> usize;
    fn row_data(&self, row: usize) -> Option<Self::Data>;
    fn set_row_data(&self, _: usize, _: Self::Data) {}
    /// Mutable models must change this version when a row changes.
    fn revision(&self) -> u64 {
        0
    }
    fn with_row(&self, row: usize, visitor: &mut dyn FnMut(&Self::Data)) {
        if let Some(value) = self.row_data(row) {
            visitor(&value);
        }
    }
    /// Return false to stop; do not mutate this model while visiting borrowed rows.
    fn visit(&self, visitor: &mut dyn FnMut(usize, &Self::Data) -> bool) {
        for row in 0..self.row_count() {
            let mut keep_going = true;
            self.with_row(row, &mut |value| {
                keep_going = visitor(row, value);
            });
            if !keep_going {
                break;
            }
        }
    }
    fn iter(&self) -> impl Iterator<Item = Self::Data>
    where
        Self: Sized,
    {
        (0..self.row_count()).filter_map(|row| self.row_data(row))
    }
}
pub struct ModelRc<T: Clone + 'static>(Rc<dyn Model<Data = T>>);
impl<T: Clone + 'static> Clone for ModelRc<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<T: Clone + 'static> Default for ModelRc<T> {
    fn default() -> Self {
        Self::new(VecModel::from(Vec::new()))
    }
}
impl<T: Clone + 'static> std::fmt::Debug for ModelRc<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModelRc")
            .field("rows", &self.row_count())
            .finish()
    }
}
impl<T: Clone + 'static> ModelRc<T> {
    pub fn new(model: impl Model<Data = T>) -> Self {
        Self(Rc::new(model))
    }
    pub fn as_any(&self) -> &dyn Any {
        self.0.as_ref()
    }
    pub fn ptr_eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
    pub fn downgrade(&self) -> std::rc::Weak<dyn Model<Data = T>> {
        Rc::downgrade(&self.0)
    }
    pub fn find_row(&self, predicate: impl Fn(&T) -> bool) -> Option<T> {
        let mut found = None;
        self.visit(&mut |_, row| {
            if predicate(row) {
                found = Some(row.clone());
                false
            } else {
                true
            }
        });
        found
    }
    pub fn iter(&self) -> impl Iterator<Item = T> {
        (0..self.row_count()).filter_map(|row| self.row_data(row))
    }
}
impl<T: Clone + 'static> Model for ModelRc<T> {
    type Data = T;
    fn row_count(&self) -> usize {
        self.0.row_count()
    }
    fn row_data(&self, row: usize) -> Option<T> {
        self.0.row_data(row)
    }
    fn set_row_data(&self, row: usize, data: T) {
        self.0.set_row_data(row, data);
    }
    fn revision(&self) -> u64 {
        self.0.revision()
    }
    fn with_row(&self, row: usize, visitor: &mut dyn FnMut(&T)) {
        self.0.with_row(row, visitor);
    }
    fn visit(&self, visitor: &mut dyn FnMut(usize, &T) -> bool) {
        self.0.visit(visitor);
    }
}
impl<T: Clone + 'static, M: Model<Data = T>> From<Rc<M>> for ModelRc<T> {
    fn from(model: Rc<M>) -> Self {
        Self(model)
    }
}
#[derive(Default)]
pub struct VecModel<T: Clone + 'static> {
    data: RefCell<Vec<T>>,
    revision: Cell<u64>,
}
impl<T: Clone + 'static> VecModel<T> {
    pub fn with_rows<R>(&self, read: impl FnOnce(&[T]) -> R) -> R {
        read(&self.data.borrow())
    }
}
impl<T: Clone + 'static> From<Vec<T>> for VecModel<T> {
    fn from(rows: Vec<T>) -> Self {
        Self {
            data: RefCell::new(rows),
            revision: Cell::new(0),
        }
    }
}
impl<T: Clone + 'static> Model for VecModel<T> {
    type Data = T;
    fn row_count(&self) -> usize {
        self.data.borrow().len()
    }
    fn row_data(&self, row: usize) -> Option<T> {
        self.data.borrow().get(row).cloned()
    }
    fn set_row_data(&self, row: usize, data: T) {
        if let Some(value) = self.data.borrow_mut().get_mut(row) {
            *value = data;
            self.revision.set(self.revision.get().wrapping_add(1));
        }
    }
    fn revision(&self) -> u64 {
        self.revision.get()
    }
    fn with_row(&self, row: usize, visitor: &mut dyn FnMut(&T)) {
        if let Some(value) = self.data.borrow().get(row) {
            visitor(value);
        }
    }
    fn visit(&self, visitor: &mut dyn FnMut(usize, &T) -> bool) {
        for (index, row) in self.data.borrow().iter().enumerate() {
            if !visitor(index, row) {
                break;
            }
        }
    }
}
#[derive(Default)]
struct RowIndex {
    version: Option<(u64, usize)>,
    positions: Vec<usize>,
}
pub struct FilterModel<T: Clone + 'static> {
    source: ModelRc<T>,
    predicate: Box<dyn Fn(&T) -> bool>,
    index: RefCell<RowIndex>,
}
impl<T: Clone + 'static> FilterModel<T> {
    pub fn new(source: ModelRc<T>, predicate: impl Fn(&T) -> bool + 'static) -> Self {
        Self {
            source,
            predicate: Box::new(predicate),
            index: RefCell::new(RowIndex::default()),
        }
    }
    fn refresh(&self) {
        let version = (self.source.revision(), self.source.row_count());
        if self.index.borrow().version == Some(version) {
            return;
        }
        let mut positions = Vec::new();
        self.source.visit(&mut |index, row| {
            if (self.predicate)(row) {
                positions.push(index);
            }
            true
        });
        *self.index.borrow_mut() = RowIndex {
            version: Some(version),
            positions,
        };
    }
}
impl<T: Clone + 'static> Model for FilterModel<T> {
    type Data = T;
    fn revision(&self) -> u64 {
        self.source.revision()
    }
    fn row_count(&self) -> usize {
        self.refresh();
        self.index.borrow().positions.len()
    }
    fn row_data(&self, row: usize) -> Option<T> {
        self.refresh();
        let position = *self.index.borrow().positions.get(row)?;
        self.source.row_data(position)
    }
    fn with_row(&self, row: usize, visitor: &mut dyn FnMut(&T)) {
        self.refresh();
        let position = self.index.borrow().positions.get(row).copied();
        if let Some(position) = position {
            self.source.with_row(position, visitor);
        }
    }
}
pub struct SortModel<T: Clone + 'static> {
    source: ModelRc<T>,
    compare: Box<dyn Fn(&T, &T) -> std::cmp::Ordering>,
    index: RefCell<RowIndex>,
}
impl<T: Clone + 'static> SortModel<T> {
    pub fn new(
        source: ModelRc<T>,
        compare: impl Fn(&T, &T) -> std::cmp::Ordering + 'static,
    ) -> Self {
        Self {
            source,
            compare: Box::new(compare),
            index: RefCell::new(RowIndex::default()),
        }
    }
    fn refresh(&self) {
        let version = (self.source.revision(), self.source.row_count());
        if self.index.borrow().version == Some(version) {
            return;
        }
        let positions = if let Some(model) = self.source.as_any().downcast_ref::<VecModel<T>>() {
            model.with_rows(|rows| {
                let mut positions = (0..rows.len()).collect::<Vec<_>>();
                positions.sort_by(|&a, &b| (self.compare)(&rows[a], &rows[b]));
                positions
            })
        } else {
            let rows = self.source.iter().collect::<Vec<_>>();
            let mut positions = (0..rows.len()).collect::<Vec<_>>();
            positions.sort_by(|&a, &b| (self.compare)(&rows[a], &rows[b]));
            positions
        };
        *self.index.borrow_mut() = RowIndex {
            version: Some(version),
            positions,
        };
    }
}
impl<T: Clone + 'static> Model for SortModel<T> {
    type Data = T;
    fn revision(&self) -> u64 {
        self.source.revision()
    }
    fn row_count(&self) -> usize {
        self.source.row_count()
    }
    fn row_data(&self, row: usize) -> Option<T> {
        self.refresh();
        let position = *self.index.borrow().positions.get(row)?;
        self.source.row_data(position)
    }
    fn with_row(&self, row: usize, visitor: &mut dyn FnMut(&T)) {
        self.refresh();
        let position = self.index.borrow().positions.get(row).copied();
        if let Some(position) = position {
            self.source.with_row(position, visitor);
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PhysicalSize {
    pub width: u32,
    pub height: u32,
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rgba8Pixel;
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SharedPixelBuffer<P = Rgba8Pixel> {
    pub width: u32,
    pub height: u32,
    pub bytes: Vec<u8>,
    marker: PhantomData<P>,
}
impl<P> SharedPixelBuffer<P> {
    #[cfg(test)]
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            bytes: vec![0; width as usize * height as usize * 4],
            marker: PhantomData,
        }
    }
    pub fn clone_from_slice(bytes: &[u8], width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            bytes: bytes.to_vec(),
            marker: PhantomData,
        }
    }
    #[cfg(test)]
    pub fn make_mut_bytes(&mut self) -> &mut [u8] {
        &mut self.bytes
    }
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Image {
    pub pixels: Arc<SharedPixelBuffer>,
    pub id: u64,
}
impl Image {
    pub fn from_rgba8(pixels: SharedPixelBuffer) -> Self {
        Self {
            pixels: Arc::new(pixels),
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
        }
    }
    pub fn size(&self) -> PhysicalSize {
        PhysicalSize {
            width: self.pixels.width,
            height: self.pixels.height,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn large_models_clone_only_requested_rows_and_cache_live_ordering() {
        struct Counted {
            value: usize,
            clones: Rc<Cell<usize>>,
        }
        impl Clone for Counted {
            fn clone(&self) -> Self {
                self.clones.set(self.clones.get() + 1);
                Self {
                    value: self.value,
                    clones: self.clones.clone(),
                }
            }
        }
        let clones = Rc::new(Cell::new(0));
        let model = Rc::new(VecModel::from(
            (0..5000)
                .map(|value| Counted {
                    value,
                    clones: clones.clone(),
                })
                .collect::<Vec<_>>(),
        ));
        let source = ModelRc::from(model.clone());
        assert_eq!(source.iter().take(30).count(), 30);
        assert_eq!(clones.replace(0), 30);
        assert_eq!(
            source.find_row(|row| row.value == 4999).unwrap().value,
            4999
        );
        assert_eq!(clones.replace(0), 1);
        let filtered = FilterModel::new(source.clone(), |row| row.value % 2 == 0);
        assert_eq!(filtered.row_count(), 2500);
        assert_eq!(clones.get(), 0);
        assert_eq!(filtered.iter().take(30).count(), 30);
        assert_eq!(clones.replace(0), 30);
        let comparisons = Rc::new(Cell::new(0));
        let sorted = SortModel::new(source, {
            let comparisons = comparisons.clone();
            move |left, right| {
                comparisons.set(comparisons.get() + 1);
                right.value.cmp(&left.value)
            }
        });
        assert_eq!(sorted.row_data(0).unwrap().value, 4999);
        let built = comparisons.get();
        assert_eq!(sorted.row_data(1).unwrap().value, 4998);
        assert_eq!(comparisons.get(), built);
        assert_eq!(clones.replace(0), 2);
        model.set_row_data(
            1,
            Counted {
                value: 10000,
                clones: clones.clone(),
            },
        );
        assert_eq!(sorted.row_data(0).unwrap().value, 10000);
        assert!(comparisons.get() > built);
        assert_eq!(filtered.row_count(), 2501);
    }

    #[test]
    fn workers_dispatch_on_the_ui_thread_and_cancelled_timers_do_not_fire() {
        let app = AppWindow::new().unwrap();
        let weak = app.as_weak();
        std::thread::spawn(move || {
            assert!(weak.upgrade().is_none());
            weak.upgrade_in_event_loop(|app| app.set_status("worker complete".into()))
                .unwrap();
        })
        .join()
        .unwrap();
        let ctx = egui::Context::default();
        pump(&ctx);
        assert_eq!(app.get_status(), "worker complete");
        let weak = app.as_weak();
        Timer::single_shot(Duration::ZERO, move || {
            weak.upgrade().unwrap().set_progress(100.0)
        });
        let timer = Timer::default();
        let weak = app.as_weak();
        timer.start(TimerMode::Repeated, Duration::ZERO, move || {
            weak.upgrade().unwrap().set_progress(-1.0)
        });
        timer.stop();
        pump(&ctx);
        assert_eq!(app.get_progress(), 100.0);
    }
    #[test]
    fn source_observers_are_reentrant_and_only_run_for_changed_values() {
        let app = AppWindow::new().unwrap();
        let calls = Rc::new(std::cell::Cell::new(0));
        let count = calls.clone();
        let weak = app.as_weak();
        app.on_sync_fetch_source(move |source| {
            count.set(count.get() + 1);
            weak.upgrade().unwrap().set_source_url(source);
        });
        app.set_source_url("https://x.com/test-fixture".into());
        app.set_source_url("https://x.com/test-fixture".into());
        assert_eq!(calls.get(), 1);
    }
}

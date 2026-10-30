use std::path::Path;

#[cfg(windows)]
pub(crate) fn read(path: &Path) -> Option<String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::{
        Win32::Storage::FileSystem::{
            GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW,
        },
        core::PCWSTR,
    };

    let filename = path
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let size = unsafe { GetFileVersionInfoSizeW(PCWSTR(filename.as_ptr()), None) };
    if size == 0 || size > 1024 * 1024 {
        return None;
    }
    let mut data = vec![0_u32; (size as usize).div_ceil(4)];
    unsafe {
        GetFileVersionInfoW(
            PCWSTR(filename.as_ptr()),
            None,
            size,
            data.as_mut_ptr().cast(),
        )
    }
    .ok()?;
    let bytes: &[u8] = bytemuck::cast_slice(&data);
    let query = |key: &str, unit_size: usize| {
        let key = key.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
        let mut pointer = std::ptr::null_mut();
        let mut length = 0;
        if !unsafe {
            VerQueryValueW(
                data.as_ptr().cast(),
                PCWSTR(key.as_ptr()),
                &mut pointer,
                &mut length,
            )
        }
        .as_bool()
        {
            return None;
        }
        // String lengths count UTF-16 characters; other resource lengths count bytes.
        let offset = (pointer as usize).checked_sub(bytes.as_ptr() as usize)?;
        let length = (length as usize).checked_mul(unit_size)?;
        bytes.get(offset..offset.checked_add(length)?)
    };
    if let Some(translations) = query("\\VarFileInfo\\Translation", 1) {
        for translation in translations.as_chunks::<4>().0 {
            let language = u16::from_le_bytes([translation[0], translation[1]]);
            let codepage = u16::from_le_bytes([translation[2], translation[3]]);
            let key = format!("\\StringFileInfo\\{language:04x}{codepage:04x}\\ProductVersion");
            if let Some(value) = query(&key, 2) {
                let value = value
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
                    .take_while(|unit| *unit != 0)
                    .collect::<Vec<_>>();
                if let Ok(version) = String::from_utf16(&value)
                    && !version.trim().is_empty()
                {
                    return Some(version.trim().to_owned());
                }
            }
        }
    }
    let fixed = query("\\", 1)?;
    let word = |offset| {
        Some(u32::from_le_bytes(
            fixed.get(offset..offset + 4)?.try_into().ok()?,
        ))
    };
    if word(0)? != 0xfeef04bd {
        return None;
    }
    let major_minor = word(16)?;
    let patch_revision = word(20)?;
    let version = format!(
        "{}.{}.{}",
        major_minor >> 16,
        major_minor & 0xffff,
        patch_revision >> 16
    );
    Some(if patch_revision & 0xffff == 0 {
        version
    } else {
        format!("{version}.{}", patch_revision & 0xffff)
    })
}

#[cfg(not(windows))]
pub(crate) fn read(_path: &Path) -> Option<String> {
    None
}

"use client";

import type { Transition, Variants } from "motion/react";
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useState, type HTMLAttributes } from "react";
import { cn } from "@/lib/utils";

type ReportIconMode = "bug" | "bulb";

interface BugReportIconProps extends HTMLAttributes<HTMLDivElement> {
    size?: number;
    loop?: boolean;
}

const LOOP_INTERVAL_MS = 2200;

const GROUP_VARIANTS: Variants = {
    hidden: {
        opacity: 0,
    },
    visible: {
        opacity: 1,
        transition: {
            duration: 0.2,
            ease: [0, 0, 0.2, 1],
        },
    },
    exit: {
        opacity: 0,
        transition: {
            duration: 0.18,
            ease: [0.4, 0, 1, 1],
        },
    },
};

const DRAW_VARIANTS: Variants = {
    hidden: {
        pathLength: 0,
        opacity: 0,
    },
    visible: {
        pathLength: 1,
        opacity: 1,
    },
    exit: {
        pathLength: 1,
        opacity: 0,
    },
};

function createDrawTransition(delay = 0, duration = 0.36): Transition {
    return {
        duration,
        delay,
        ease: [0.4, 0, 0.2, 1],
        opacity: { delay },
    };
}

function BugPaths() {
    return (<>
      <motion.path d="m8 2 1.88 1.88" transition={createDrawTransition(0)} variants={DRAW_VARIANTS}/>
      <motion.path d="M14.12 3.88 16 2" transition={createDrawTransition(0.04)} variants={DRAW_VARIANTS}/>
      <motion.path d="M9 7.13V6a3 3 0 1 1 6 0v1.13" transition={createDrawTransition(0.08)} variants={DRAW_VARIANTS}/>
      <motion.path d="M6.53 9A4 4 0 0 1 3 5" transition={createDrawTransition(0.14)} variants={DRAW_VARIANTS}/>
      <motion.path d="M17.47 9A4 4 0 0 0 21 5" transition={createDrawTransition(0.18)} variants={DRAW_VARIANTS}/>
      <motion.path d="M12 20v-9" transition={createDrawTransition(0.24)} variants={DRAW_VARIANTS}/>
      <motion.path d="M14 7a4 4 0 0 1 4 4v3a6 6 0 0 1-12 0v-3a4 4 0 0 1 4-4z" transition={createDrawTransition(0.3, 0.42)} variants={DRAW_VARIANTS}/>
      <motion.path d="M22 13h-4" transition={createDrawTransition(0.42)} variants={DRAW_VARIANTS}/>
      <motion.path d="M6 13H2" transition={createDrawTransition(0.46)} variants={DRAW_VARIANTS}/>
      <motion.path d="M21 21a4 4 0 0 0-3.81-4" transition={createDrawTransition(0.52)} variants={DRAW_VARIANTS}/>
      <motion.path d="M3 21a4 4 0 0 1 3.81-4" transition={createDrawTransition(0.56)} variants={DRAW_VARIANTS}/>
    </>);
}

function BulbPaths() {
    return (<>
      <motion.path d="M15 14c.2-1 .7-1.7 1.5-2.5 1-.9 1.5-2.2 1.5-3.5A6 6 0 0 0 6 8c0 1 .2 2.2 1.5 3.5.7.7 1.3 1.5 1.5 2.5" transition={createDrawTransition(0, 0.46)} variants={DRAW_VARIANTS}/>
      <motion.path d="M9 18h6" transition={createDrawTransition(0.16)} variants={DRAW_VARIANTS}/>
      <motion.path d="M10 22h4" transition={createDrawTransition(0.24)} variants={DRAW_VARIANTS}/>
    </>);
}

function ReportIconGroup({ mode }: { mode: ReportIconMode; }) {
    return (<motion.g animate="visible" exit="exit" initial="hidden" variants={GROUP_VARIANTS}>
      {mode === "bug" ? <BugPaths /> : <BulbPaths />}
    </motion.g>);
}

function StaticBugIcon() {
    return (<g>
      <path d="m8 2 1.88 1.88"/>
      <path d="M14.12 3.88 16 2"/>
      <path d="M9 7.13V6a3 3 0 1 1 6 0v1.13"/>
      <path d="M6.53 9A4 4 0 0 1 3 5"/>
      <path d="M17.47 9A4 4 0 0 0 21 5"/>
      <path d="M12 20v-9"/>
      <path d="M14 7a4 4 0 0 1 4 4v3a6 6 0 0 1-12 0v-3a4 4 0 0 1 4-4z"/>
      <path d="M22 13h-4"/>
      <path d="M6 13H2"/>
      <path d="M21 21a4 4 0 0 0-3.81-4"/>
      <path d="M3 21a4 4 0 0 1 3.81-4"/>
    </g>);
}

function BugReportIcon({ className, size = 28, loop = false, ...props }: BugReportIconProps) {
    const [mode, setMode] = useState<ReportIconMode>("bug");

    useEffect(() => {
        if (!loop) {
            return;
        }

        const intervalId = window.setInterval(() => {
            setMode((currentMode) => currentMode === "bug" ? "bulb" : "bug");
        }, LOOP_INTERVAL_MS);

        return () => window.clearInterval(intervalId);
    }, [loop]);

    return (<div className={cn("flex items-center justify-center", className)} {...props}>
      <svg fill="none" height={size} stroke="currentColor" strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" viewBox="0 0 24 24" width={size} xmlns="http://www.w3.org/2000/svg">
        {loop ? (<AnimatePresence>
            <ReportIconGroup key={mode} mode={mode} />
          </AnimatePresence>) : (<StaticBugIcon />)}
      </svg>
    </div>);
}

export { BugReportIcon };

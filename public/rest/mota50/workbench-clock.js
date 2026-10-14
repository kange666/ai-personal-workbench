/* ASTRION: pause this game's timers and animation clock without discarding its state. */
(function () {
    "use strict";
    var nativeTimeout = window.setTimeout.bind(window), nativeClear = window.clearTimeout.bind(window);
    var nativeFrame = window.requestAnimationFrame.bind(window), nativeCancel = window.cancelAnimationFrame.bind(window);
    var tasks = new Map(), nextId = 1, paused = false, pausedAt = 0, offset = 0;
    function schedule(task) {
        if (paused) return;
        task.started = performance.now();
        if (task.kind === "frame") task.handle = nativeFrame(function (time) { fire(task, time - offset); });
        else task.handle = nativeTimeout(function () { fire(task); }, task.remaining);
    }
    function fire(task, time) {
        task.handle = null;
        if (!tasks.has(task.id) || paused) return;
        if (task.kind !== "interval") tasks.delete(task.id);
        try {
            if (typeof task.callback === "function") task.callback.apply(window, task.kind === "frame" ? [time] : task.args);
            else if (typeof task.callback === "string") Function(task.callback)();
        } finally {
            if (task.kind === "interval" && tasks.has(task.id)) { task.remaining = task.delay; schedule(task); }
        }
    }
    function add(kind, callback, delay, args) {
        var task = { id: nextId++, kind: kind, callback: callback, delay: Math.max(0, Number(delay) || 0), args: args, handle: null };
        task.remaining = task.delay;
        tasks.set(task.id, task); schedule(task); return task.id;
    }
    function clear(id) {
        var task = tasks.get(id);
        if (!task) return;
        if (task.kind === "frame") nativeCancel(task.handle); else nativeClear(task.handle);
        tasks.delete(id);
    }
    window.setTimeout = function (callback, delay) { return add("timeout", callback, delay, Array.prototype.slice.call(arguments, 2)); };
    window.setInterval = function (callback, delay) { return add("interval", callback, delay, Array.prototype.slice.call(arguments, 2)); };
    window.clearTimeout = window.clearInterval = window.cancelAnimationFrame = clear;
    window.requestAnimationFrame = function (callback) { return add("frame", callback, 0, []); };
    window.MotaWorkbenchClock = {
        pause: function () {
            if (paused) return;
            pausedAt = performance.now(); paused = true;
            tasks.forEach(function (task) {
                if (task.handle == null) return;
                if (task.kind === "frame") nativeCancel(task.handle);
                else { nativeClear(task.handle); task.remaining = Math.max(0, task.remaining - (pausedAt - task.started)); }
                task.handle = null;
            });
        },
        resume: function () {
            if (!paused) return;
            offset += performance.now() - pausedAt; paused = false;
            tasks.forEach(schedule);
        }
    };
})();

/* ASTRION local integration. Original maps, combat values and story remain unchanged. */
(function () {
    "use strict";
    var channel = "astrion-mota50-v1", ready = false, paused = true;
    function send(type, extra) {
        parent.postMessage(Object.assign({ channel: channel, type: type }, extra || {}), location.origin);
    }
    function pause() {
        paused = true;
        if (!ready) return;
        core.status.holdingKeys = [];
        core.stopAutomaticRoute();
        core.control.checkAutosave();
        core.pauseBgm(); core.stopSound();
        if (core.musicStatus.audioContext) core.musicStatus.audioContext.suspend().catch(function () {});
        window.MotaWorkbenchClock.pause();
    }
    function resume() {
        paused = false;
        if (!ready) return;
        window.MotaWorkbenchClock.resume();
        if (core.musicStatus.audioContext) core.musicStatus.audioContext.resume().catch(function () {});
        core.resumeBgm(true);
    }
    window.addEventListener("message", function (event) {
        if (event.source !== parent || event.origin !== location.origin || !event.data || event.data.channel !== channel) return;
        switch (event.data.type) {
            case "status": if (ready) send("ready"); break;
            case "pause": pause(); break;
            case "resume": resume(); break;
            case "save":
                if (!ready || paused) return;
                if (!core.isPlaying()) { core.drawTip("请先开始游戏，再选择存档。"); return; }
                core.save(); break;
            case "load": if (ready && !paused) core.load(); break;
        }
    });
    window.addEventListener("keydown", function (event) {
        if (event.key === "Tab") {
            event.preventDefault(); event.stopImmediatePropagation(); send("tab", { shiftKey: event.shiftKey });
        } else if (event.key === "Escape" && (!ready || !core.status.event || !core.status.event.id)) {
            event.preventDefault(); event.stopImmediatePropagation(); send("escape");
        } else if (paused) { event.preventDefault(); event.stopImmediatePropagation(); }
    }, true);
    document.addEventListener("visibilitychange", function () { if (document.hidden) pause(); });
    // Send local asset errors to the visible game component instead of leaving a blank frame.
    window.addEventListener("error", function (event) {
        var target = event.target;
        if (target && (target.tagName === "SCRIPT" || target.tagName === "IMG")) send("error", { message: "本地游戏资源加载失败，请重新加载。" });
    }, true);

    var loadJs = main.loadJs;
    main.loadJs = function (dir, list, callback) {
        loadJs.call(main, dir, list, function () {
            if (dir === "project") {
                // Use an independent storage namespace, so other towers and 21-floor progress never overlap.
                data_a1e2fb4a_e986_4524_b0da_9b7ba7c0874d.firstData.name = "astrion-mota50-v1";
            } else if (dir === "libs") {
                // The upstream cache changes before the disk write; restore it if storage fails.
                var writeSave = utils.prototype._setLocalForage_set;
                utils.prototype._setLocalForage_set = function (name, text, callback) {
                    var previous = core.saves.cache[name];
                    writeSave.call(this, name, text, function (error) {
                        if (error) {
                            if (previous == null) delete core.saves.cache[name];
                            else core.saves.cache[name] = previous;
                        }
                        callback(error);
                    });
                };
                var http = utils.prototype.http;
                utils.prototype.http = function (method, url, body, success, error, mime, responseType, progress) {
                    var resolved = new URL(url, location.href);
                    if (method.toUpperCase() !== "GET" || resolved.origin !== location.origin || !resolved.pathname.startsWith(new URL("./", location.href).pathname)) {
                        if (error) error(new Error("离线游戏仅支持本机存档。"));
                        return;
                    }
                    return http.call(this, method, url, body, success, error, mime, responseType, progress);
                };
                events.prototype._startGame_upload = function () {};
                events.prototype._gameOver_confirmUpload = function (ending) { core.events._gameOver_confirmDownload(ending); };
                control.prototype.syncSave = control.prototype.syncLoad = function () { core.drawTip("离线版请使用本机存档或存档文件。"); };
            }
            callback();
        });
    };
    // Load the bundled floor files directly; there is no remote floor-combining endpoint offline.
    main.loadFloors = function (callback) {
        var count = 0;
        main.floorIds.forEach(function (id) {
            main.loadFloor(id, function () {
                if (++count === main.floorIds.length) { main.dom.mainTips.style.display = "none"; callback(); }
            });
        });
    };
    main.init("play", function () {
        ready = true;
        var hint = document.getElementById("startTopHint");
        if (hint) hint.textContent = "经典 50 层 · 离线游戏 · 存档保存在本机";
        if (paused) pause();
        send("ready");
    });
    main.listen();
})();

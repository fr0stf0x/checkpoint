(function () {
  'use strict';
  const invoke = window.__TAURI__ ? window.__TAURI__.core.invoke : null;
  const $ = (s) => document.querySelector(s);
  const pane = $('#pane'), layer = $('#layer');
  const esc = (s) => String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));

  // Game list layout ('grid' | 'list'): a per-PC convenience, so browser storage is enough.
  const store = {
    get(k, d) { try { return localStorage.getItem(k) || d; } catch (e) { return d; } },
    set(k, v) { try { localStorage.setItem(k, v); } catch (e) { /* storage blocked: keep in memory */ } },
  };
  const S = { info: null, list: null, accounts: null, view: 'games', filter: 'all', q: '', loading: false, busy: false, lang: 'en', T: {}, update: null, updateHidden: false, layout: store.get('cp.layout', 'grid') === 'list' ? 'list' : 'grid' };

  // ---------- icons ----------
  // Own 24×24 line icons (stroke = currentColor), bundled here so the app works offline.
  const CLOUD = '<path d="M7 18a5 5 0 1 1 .9-9.9A6 6 0 0 1 19 10a4 4 0 0 1-1 8"/>';
  const ICONS = {
    gamepad: '<path d="M6 11h4M8 9v4M15 12h.01M18 10h.01"/><path d="M17.3 5H6.7a4 4 0 0 0-3.96 3.4l-1.1 7.6A2.6 2.6 0 0 0 4.2 19c.9 0 1.7-.5 2.1-1.3L7.5 15h9l1.2 2.7c.4.8 1.2 1.3 2.1 1.3a2.6 2.6 0 0 0 2.56-3l-1.1-7.6A4 4 0 0 0 17.3 5z"/>',
    users: '<circle cx="9" cy="8" r="4"/><path d="M2 21v-1a6 6 0 0 1 6-6h2a6 6 0 0 1 6 6v1M16 4.1a4 4 0 0 1 0 7.8M22 21v-1a6 6 0 0 0-4-5.6"/>',
    history: '<path d="M3 12a9 9 0 1 0 3-6.7L3 8"/><path d="M3 3v5h5M12 7v5l3 3"/>',
    sliders: '<path d="M4 6h10M18 6h2M4 12h4M12 12h8M4 18h12"/><circle cx="16" cy="6" r="2"/><circle cx="10" cy="12" r="2"/><circle cx="18" cy="18" r="2"/>',
    bug: '<rect x="7" y="7" width="10" height="13" rx="5"/><path d="M12 11v9M9 4l1.5 2M15 4l-1.5 2M3 13h4M17 13h4M4 8l3 2M20 8l-3 2M4 20l3-2M20 20l-3-2"/>',
    upload: CLOUD + '<path d="M12 12v9M9 15l3-3 3 3"/>',
    download: CLOUD + '<path d="M12 12v9M9 18l3 3 3-3"/>',
    cloud: '<path d="M7 19a5 5 0 1 1 .9-9.9A6 6 0 0 1 19 11a4 4 0 0 1-1 8z"/>',
    refresh: '<path d="M21 12a9 9 0 0 1-15.5 6.2L3 16M3 21v-5h5"/><path d="M3 12a9 9 0 0 1 15.5-6.2L21 8M21 3v5h-5"/>',
    swap: '<path d="M4 8h15l-4-4M20 16H5l4 4"/>',
    search: '<circle cx="11" cy="11" r="7"/><path d="m20 20-4-4"/>',
    grid: '<rect x="3" y="3" width="7" height="7" rx="1.5"/><rect x="14" y="3" width="7" height="7" rx="1.5"/><rect x="3" y="14" width="7" height="7" rx="1.5"/><rect x="14" y="14" width="7" height="7" rx="1.5"/>',
    list: '<path d="M8 6h13M8 12h13M8 18h13M3.5 6h.01M3.5 12h.01M3.5 18h.01"/>',
    check: '<path d="m5 12 5 5L20 7"/>',
    power: '<path d="M12 3v9"/><path d="M6.3 6.3a8 8 0 1 0 11.4 0"/>',
    alert: '<path d="M10.3 3.9 2.4 18a2 2 0 0 0 1.7 3h15.8a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z"/><path d="M12 9v4M12 17h.01"/>',
    monitor: '<rect x="3" y="4" width="18" height="12" rx="2"/><path d="M8 20h8M12 16v4"/>',
    plus: '<circle cx="12" cy="12" r="9"/><path d="M12 8v8M8 12h8"/>',
    minus: '<circle cx="12" cy="12" r="9"/><path d="M8 12h8"/>',
    help: '<circle cx="12" cy="12" r="9"/><path d="M9.5 9a2.5 2.5 0 1 1 3.5 2.3c-.6.3-1 .9-1 1.6V14M12 17h.01"/>',
    key: '<circle cx="8" cy="15" r="4"/><path d="m11 12 9-9M17 6l3 3M15 8l2 2"/>',
    play: '<path d="M7 4v16l13-8z"/>',
    sparkle: '<path d="M12 3l1.8 4.7 4.7 1.8-4.7 1.8L12 16l-1.8-4.7-4.7-1.8 4.7-1.8z"/><path d="M19 15l.8 2.2 2.2.8-2.2.8L19 21l-.8-2.2-2.2-.8 2.2-.8z"/>',
    save: '<path d="M5 3h11l3 3v13a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2z"/><path d="M8 3v5h7M8 21v-7h8v7"/>',
    folder: '<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>',
    copy: '<rect x="9" y="9" width="12" height="12" rx="2"/><path d="M5 15H4a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1h10a1 1 0 0 1 1 1v1"/>',
    external: '<path d="M14 4h6v6M20 4l-9 9M18 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h5"/>',
    eye: '<path d="M2 12s3.6-7 10-7 10 7 10 7-3.6 7-10 7S2 12 2 12z"/><circle cx="12" cy="12" r="3"/>',
    eyeOff: '<path d="M10.6 5.1A10.7 10.7 0 0 1 12 5c6.4 0 10 7 10 7a17.6 17.6 0 0 1-2.9 3.8M6.6 6.6C3.8 8.4 2 12 2 12s3.6 7 10 7a9.8 9.8 0 0 0 5.4-1.6M9.9 9.9a3 3 0 0 0 4.2 4.2M3 3l18 18"/>',
    trash: '<path d="M4 7h16M10 11v6M14 11v6M6 7l1 12a2 2 0 0 0 2 2h6a2 2 0 0 0 2-2l1-12M9 7V4h6v3"/>',
    more: '<circle cx="5" cy="12" r="1.2"/><circle cx="12" cy="12" r="1.2"/><circle cx="19" cy="12" r="1.2"/>',
    library: '<rect x="3" y="4" width="4" height="16" rx="1"/><rect x="9" y="4" width="4" height="16" rx="1"/><path d="m15.5 5.5 3.8-1 3 14.6-3.8 1z"/>',
  };
  const ic = (name) => `<svg class="ic" viewBox="0 0 24 24" aria-hidden="true" focusable="false">${ICONS[name] || ''}</svg>`;
  const STATUS_IC = { sync: 'check', local: 'monitor', cloud: 'cloud', conflict: 'alert', diverged: 'alert', new: 'plus', empty: 'minus', unknown: 'help' };

  // ---------- i18n ----------
  // Strings come from src-tauri/locales/<lang>.yml (the "ui" subtree), loaded through get_strings.
  // t(): plain text, values inserted as-is. h(): for HTML, values are escaped. Translations may contain markup.
  function fill(s, vars, escape) {
    return s.replace(/%\{(\w+)\}/g, (m, k) => (vars && k in vars ? (escape ? esc(vars[k]) : String(vars[k])) : m));
  }
  function pick(key, vars) {
    if (vars && typeof vars.n === 'number') {
      const one = S.T[key + '_one'];
      if (vars.n === 1 && one !== undefined) return one;
      if (S.T[key + '_other'] !== undefined) return S.T[key + '_other'];
    }
    return S.T[key] !== undefined ? S.T[key] : key;
  }
  const t = (key, vars) => fill(pick(key, vars), vars, false);
  const h = (key, vars) => fill(pick(key, vars), vars, true);
  const locale = () => (S.lang === 'vi' ? 'vi-VN' : 'en-US');

  async function loadStrings() {
    if (!invoke) return;
    const r = await invoke('get_strings');
    S.lang = r.lang; S.T = r.strings;
    document.documentElement.lang = S.lang;
    document.querySelectorAll('[data-i18n]').forEach((el) => { el.textContent = t(el.dataset.i18n); });
    // On narrow windows the sidebar shows icons only: keep each button's name for tooltips and screen readers.
    document.querySelectorAll('#nav button').forEach((b) => {
      const l = b.querySelector('.navlbl');
      if (l) { b.title = l.textContent; b.setAttribute('aria-label', l.textContent); }
    });
  }

  // ---------- helpers ----------
  function fmt(ms) {
    if (!ms) return '—';
    return new Date(ms).toLocaleString(locale(), { dateStyle: 'short', timeStyle: 'short' });
  }
  function ago(ms) {
    if (!ms) return '';
    const rtf = new Intl.RelativeTimeFormat(locale(), { numeric: 'auto' });
    const m = Math.round((ms - Date.now()) / 60000);
    if (Math.abs(m) < 1) return t('time.just_now');
    if (Math.abs(m) < 60) return rtf.format(m, 'minute');
    const hrs = Math.round(m / 60);
    if (Math.abs(hrs) < 48) return rtf.format(hrs, 'hour');
    return rtf.format(Math.round(hrs / 24), 'day');
  }
  function diff(ms) {
    const m = Math.round(Math.abs(ms) / 60000), hr = Math.floor(m / 60), d = Math.floor(hr / 24);
    if (d >= 2) return t('time.days', { n: d });
    return hr ? t('time.hours_minutes', { h: hr, m: m % 60 }) : t('time.minutes', { n: m });
  }
  function human(b) {
    if (!b) return '0 B';
    if (b >= 1e9) return (b / 1e9).toFixed(2) + ' GB';
    if (b >= 1e6) return (b / 1e6).toFixed(1) + ' MB';
    if (b >= 1e3) return Math.round(b / 1e3) + ' KB';
    return b + ' B';
  }
  const files = (n) => t('common.files', { n });
  // action: { label, run } adds a button (e.g. Undo) and keeps the toast up longer
  function toast(msg, err, action) {
    const el = document.createElement('div');
    el.className = 'toast' + (err ? ' err' : '') + (action ? ' act' : '');
    const text = document.createElement('span');
    text.textContent = msg;
    el.appendChild(text);
    if (action) {
      const b = document.createElement('button');
      b.className = 'btn sm';
      b.textContent = action.label;
      b.addEventListener('click', () => { el.remove(); action.run(); });
      el.appendChild(b);
    }
    $('#toasts').appendChild(el);
    setTimeout(() => el.remove(), action ? 12000 : err ? 7000 : 3500);
  }
  async function call(cmd, args) {
    if (!invoke) throw new Error('Checkpoint must run inside the desktop app (Tauri).');
    return invoke(cmd, args || {});
  }
  function close() { layer.innerHTML = ''; restoreFocus(); }
  function modal(html) {
    // Remember where the focus was, to go back there when the dialog closes
    if (!layer.querySelector('.modal') && !layer.contains(document.activeElement)) S.lastKey = keyOf(document.activeElement) || S.lastKey;
    layer.innerHTML = `<div class="scrim" data-scrim="1"><div class="modal" role="dialog" aria-modal="true">${html}</div></div>`;
    // data-autofocus wins, so a dialog that deletes something starts on Cancel (safe for A / Enter)
    const b = layer.querySelector('.modal [data-autofocus]') || layer.querySelector('.modal .primary, .modal .danger, .modal button');
    if (b) b.focus();
  }
  function busyModal(title) {
    modal(`<h3>${esc(title)}</h3><div class="loading"><span class="spin"></span><span id="busyText">${h('common.working')}</span></div>`);
  }
  // Backup/restore progress in one list. A restore lists its steps up front (keep the save on
  // this PC, download, write files); each step row turns from waiting to spinner to check, and the
  // save files show under the step that packs or writes them. A backup lists the files being
  // packed, then adds the upload row. Returns the Channel to pass to the command (onProgress) and
  // done() to show every row finished.
  function progressModal(title, sub) {
    modal(`<h3>${esc(title)}</h3>${sub ? `<span class="note">${esc(sub)}</span>` : ''}
      <div class="prog-head"><span id="pgStep">${h('common.working')}</span><span class="spacer"></span><b id="pgCount"></b></div>
      <div class="bar"><i id="pgBar"></i></div>
      <div class="files" id="pgList" hidden></div>`);
    // Look up rows in this dialog only: a late message for the previous game of a multi-game run
    // must not touch the next game's list
    const root = layer.querySelector('.modal');
    const q = (sel) => root.querySelector(sel);
    const LABEL = { snapshot: 'progress.step_snapshot', pack: 'progress.step_pack', upload: 'progress.step_upload', download: 'progress.step_download', extract: 'progress.step_extract' };
    const ROW = { snapshot: 'progress.snapshot_row', upload: 'progress.upload_row', download: 'progress.download_row', extract: 'progress.extract_row' };
    const ICON = { snapshot: 'history', upload: 'upload', download: 'download', extract: 'save' };
    let steps = [], info = {}, cur = null, items = [], fi = -1, total = {};
    const row = (s) => q('#pgS-' + s);
    const size = (s, text) => { const z = q('#pgS-' + s + ' .sz'); if (z) z.textContent = text; };
    // Whole bar across the steps of a restore; per step for a backup
    const bar = (f) => {
      const i = steps.indexOf(cur);
      const all = steps.length && i >= 0 ? (i + Math.min(1, f)) / steps.length : f;
      const b = q('#pgBar'); if (b) b.style.width = Math.round(Math.min(1, all) * 100) + '%';
    };
    const mark = (el, st) => {
      if (!el) return;
      el.classList.remove('wait', 'active', 'done');
      el.classList.add(st);
      el.querySelector('.st').innerHTML = st === 'active' ? '<span class="spin"></span>' : st === 'done' ? ic('check') : '<span class="dot"></span>';
      if (st === 'active') el.scrollIntoView({ block: 'nearest' });
    };
    // Middle ellipsis: the start of the folder shrinks, the last two folders always show
    const where = (dir) => {
      const parts = dir.match(/[^\\/]*[\\/]?/g).filter(Boolean);
      const cut = Math.max(0, parts.length - 2);
      return `<span class="h">${esc(parts.slice(0, cut).join(''))}</span><span class="t">${esc(parts.slice(cut).join(''))}</span>`;
    };
    const stepRow = (s, x) => {
      let note = '';
      if (s === 'snapshot') note = `${files(x.files)} · ${human(x.bytes)} · ${t('progress.snapshot_sub')}`;
      else if (s === 'download' && x.device) note = `${x.device} · ${fmt(x.time)}`;
      else if (s === 'extract') note = `${files(x.files)} · ${human(x.bytes)}`;
      return `<div class="file xfer wait" id="pgS-${s}"><span class="st"><span class="dot"></span></span>
        <span class="nm"><span class="fn">${ic(ICON[s])}${h(ROW[s])}</span>${note ? `<span class="sub">${esc(note)}</span>` : ''}</span><span class="sz"></span></div>`;
    };
    const countFiles = () => {
      const n = items.filter((f) => f.done).length;
      if (row(cur)) size(cur, `${n}/${items.length}`);
      else { const c = q('#pgCount'); if (c) c.textContent = `${n}/${items.length}`; }
    };
    const finishFiles = () => { items.forEach((f, i) => { if (!f.done) { f.done = true; mark(q('#pgF' + i), 'done'); } }); fi = -1; if (items.length) countFiles(); };
    const finishStep = (s) => {
      if (s === 'pack' || s === 'extract') finishFiles();
      const r = row(s); if (!r) return;
      mark(r, 'done');
      if (total[s]) size(s, human(total[s]));
      else if (info[s] && info[s].bytes) size(s, human(info[s].bytes));
    };
    const ch = new window.__TAURI__.core.Channel();
    ch.onmessage = (m) => {
      const l = q('#pgList'); if (!l || !root.isConnected) return;
      if (m.kind === 'steps') {
        steps = m.steps.map((x) => x.step);
        m.steps.forEach((x) => { info[x.step] = x; });
        l.insertAdjacentHTML('beforeend', m.steps.map((x) => stepRow(x.step, x)).join(''));
        l.hidden = false;
      } else if (m.kind === 'step') {
        const lb = q('#pgStep'); if (lb && LABEL[m.step]) lb.textContent = t(LABEL[m.step]);
        if (cur) finishStep(cur);
        cur = m.step;
        // A backup's upload row is added when the upload starts
        if (!row(cur) && (cur === 'upload' || cur === 'download')) { l.insertAdjacentHTML('beforeend', stepRow(cur, {})); l.hidden = false; }
        mark(row(cur), 'active');
        const i = steps.indexOf(cur);
        const c = q('#pgCount'); if (c && i >= 0) c.textContent = `${i + 1}/${steps.length}`;
        bar(0);
      } else if (m.kind === 'files') {
        items = m.files.map((f) => ({ ...f, done: false }));
        const under = !!row(cur);
        l.hidden = l.hidden && !items.length;
        l.insertAdjacentHTML('beforeend', items.map((f, i) => `<div class="file wait${under ? ' sub' : ''}" id="pgF${i}" title="${esc(f.dir + f.name)}"><span class="st"><span class="dot"></span></span>
          <span class="nm"><span class="fn">${esc(f.name)}</span><span class="fp">${where(f.dir)}</span></span><span class="sz">${human(f.size)}</span></div>`).join(''));
        countFiles();
      } else if (m.kind === 'file') {
        if (fi >= 0 && items[fi]) { items[fi].done = true; mark(q('#pgF' + fi), 'done'); }
        fi = m.index;
        mark(q('#pgF' + fi), 'active');
        countFiles();
        bar(fi / Math.max(1, items.length));
      } else if (m.kind === 'count') {
        size(cur, `${m.done}/${m.total}`);
        bar(m.done / Math.max(1, m.total));
      } else if (m.kind === 'bytes') {
        total[cur] = m.total;
        size(cur, m.total ? `${Math.round((100 * m.done) / m.total)}% · ${human(m.done)} / ${human(m.total)}` : human(m.done));
        if (m.total) bar(m.done / m.total);
      }
    };
    const done = () => {
      if (cur) finishStep(cur);
      finishFiles();
      cur = null;
      const b = q('#pgBar'); if (b) b.style.width = '100%';
      const lb = q('#pgStep'); if (lb) lb.innerHTML = `<span class="ok-text">${h('progress.done')}</span>`;
    };
    return { ch, done };
  }
  const backendReady = () => S.info && S.info.config.backend.kind !== 'none';

  // ---------- data ----------
  async function loadInfo() {
    S.info = await call('get_info');
    renderSide();
  }
  async function loadGames() {
    S.loading = true; render();
    try { S.list = await call('list_games'); }
    catch (e) { S.list = null; fail(t('err.scan_failed'), e); }
    try { S.pending = await call('migrate_pending'); } catch (e) { S.pending = []; }
    S.loading = false;
    $('#cGames').textContent = S.list ? S.list.games.length : '';
    render();
  }
  // Games are started and closed while the app sits in the background, so the "running" flags and
  // local saves go stale: rescan quietly (no loading state) when the window comes back, at most
  // every 5 s, and not while a dialog is open or something is in progress.
  let focusScanAt = 0;
  async function rescanOnReturn() {
    const idle = () => S.view === 'games' && S.list && !S.loading && !S.busy && !layer.querySelector('.modal');
    if (document.hidden || !idle() || Date.now() - focusScanAt < 5000) return;
    focusScanAt = Date.now();
    let list, pending;
    try { list = await call('list_games'); pending = await call('migrate_pending'); } catch (e) { return; }
    if (!idle()) return;
    S.list = list; S.pending = pending;
    $('#cGames').textContent = list.games.length;
    render();
  }
  window.addEventListener('focus', rescanOnReturn);
  document.addEventListener('visibilitychange', rescanOnReturn);

  async function loadAccounts() {
    try { S.accounts = await call('list_accounts'); $('#cAcc').textContent = S.accounts.length; }
    catch (e) { S.accounts = []; }
  }

  function renderSide() {
    const i = S.info; if (!i) return;
    const b = i.config.backend;
    const where = b.kind === 'folder' ? b.path : b.kind === 'rclone' ? b.remote : t('side.not_connected');
    const ver = $('#appVer');
    if (ver) { ver.textContent = (/^\d/.test(i.version) ? 'v' : '') + i.version; ver.title = t('side.version', { version: i.version }); }
    $('#sideMeta').innerHTML = `
      <div>${h('side.this_pc')}<br><span class="mono">${esc(i.config.device_name)}</span></div>
      <div>${h('side.storage')}<br><span class="mono">${esc(where)}</span></div>
      <div>${h('side.manifest')}<br><span class="mono">${i.manifest_games ? h('side.manifest_games', { n: i.manifest_games.toLocaleString(locale()) }) : h('side.manifest_missing')}</span></div>`;
    // Shown only while a controller (or the arrow keys) is in use: body.pad
    const key = (k, cls, label) => `<div><span class="pk ${cls}">${k}</span>${h(label)}</div>`;
    $('#padHints').innerHTML = key('A', 'pa', 'pad.a') + key('B', 'pb', 'pad.b') + key('X', 'px', 'pad.x') + key('LB RB', 'pl', 'pad.lbrb');
  }

  // ---------- views ----------
  function render() {
    document.querySelectorAll('#nav button').forEach((b) => b.classList.toggle('on', b.dataset.view === S.view));
    ({ games: renderGames, accounts: renderAccounts, history: renderHistory, settings: renderSettings })[S.view]();
    restoreFocus();
  }

  function banner(kind, icon, text, buttons) {
    return `<div class="banner ${kind}">${ic(icon)}<p>${text}</p>${buttons}</div>`;
  }
  function banners() {
    const out = [];
    if (S.update && !S.updateHidden) {
      const u = S.update;
      out.push(banner('accent', 'sparkle', h('update.available', { version: u.version, current: u.current }),
        `<button class="btn" data-act="updLater">${h('update.later')}</button><button class="btn primary" data-act="updShow">${h('update.whats_new')}</button>`));
    }
    (S.pending || []).forEach((m) => {
      const v = { game: m.game_name, from: acctName(m.from), to: acctName(m.to) };
      out.push(banner('warn', 'swap', h('games.pending_move', v),
        `<button class="btn" data-act="migUndo" data-mid="${esc(m.id)}">${h('move.undo_btn')}</button><button class="btn primary" data-act="migKeep" data-mid="${esc(m.id)}">${h('move.keep_btn')}</button>`));
    });
    if (S.info && !S.info.manifest_games) {
      out.push(banner('warn', 'alert', h('games.no_manifest'), `<button class="btn primary" data-act="manifest">${ic('download')}${h('games.download_manifest')}</button>`));
    }
    if (!backendReady()) {
      out.push(banner('warn', 'cloud', h('games.no_backend'), `<button class="btn" data-view="settings">${ic('sliders')}${h('games.open_settings')}</button>`));
    } else if (S.list && S.list.cloud_error) {
      out.push(banner('bad', 'alert', h('games.cloud_error', { detail: S.list.cloud_error }), `<button class="btn" data-act="reload">${ic('refresh')}${h('common.retry')}</button>`));
    }
    return out.join('');
  }

  // Steam's cached avatars / artwork (served by the backend's steamimg protocol). The first letter
  // stays underneath as a placeholder; a missing image (404) removes the <img> and shows it.
  function art(kind, id, name, cls) {
    const conv = window.__TAURI__ && window.__TAURI__.core && window.__TAURI__.core.convertFileSrc;
    const src = conv ? conv(`${kind}/${id}`, 'steamimg') : '';
    const first = ([...String(name || '?').trim()][0] || '?').toUpperCase();
    return `<span class="art ${cls}" aria-hidden="true"><b>${esc(first)}</b>${src ? `<img src="${esc(src)}" alt="" loading="lazy" onerror="this.remove()">` : ''}</span>`;
  }

  function statusPill(k, label) { return `<span class="pill s-${k}">${ic(STATUS_IC[k] || 'help')}${esc(label || t('status.' + k))}</span>`; }

  // Backup / Restore / Move buttons of one game. iconOnly for the grid cards (label in title + aria-label).
  function gameActions(g, iconOnly) {
    const dis = !backendReady() || g.running || S.busy ? 'disabled' : '';
    const b = (act, icon, label, state, hint) => iconOnly
      ? `<button class="btn icon" data-act="${act}" data-id="${g.appid}" title="${esc(hint || label)}" aria-label="${esc(label)}" ${state}>${ic(icon)}</button>`
      : `<button class="btn sm" data-act="${act}" data-id="${g.appid}" ${hint ? `title="${esc(hint)}"` : ''} ${state}>${ic(icon)}${esc(label)}</button>`;
    return b('backup', 'upload', t('common.backup'), g.local_hash ? dis : 'disabled')
      + b('restore', 'download', t('common.restore'), g.cloud ? dis : 'disabled')
      + (g.steam_ids && g.steam_ids.length ? b('migrate', 'swap', t('games.move_btn'), g.running || S.busy ? 'disabled' : '', t('games.move_title')) : '')
      + b('gameMore', 'more', t('game.more'), '', t('game.more_title'));
  }
  function gameFlags(g) {
    return (g.steamid_bound ? `<span class="flag">${ic('key')}${h('games.flag_steamid')}</span>` : '')
      + (g.running ? `<span class="flag run">${ic('play')}${h('games.flag_running')}</span>` : '');
  }
  function gameWhere(g) {
    return g.roots.length ? g.roots.join('\n') : g.has_rules ? t('games.no_save_here') : t('games.not_in_manifest');
  }

  // ---------- selection (several games for Backup / Restore) ----------
  const sel = () => (S.sel || (S.sel = new Set()));
  function pickBox(g) {
    const label = h('games.select', { game: g.name });
    return `<label class="pick" title="${label}"><input type="checkbox" data-pick="${g.appid}" ${sel().has(g.appid) ? 'checked' : ''} aria-label="${label}"></label>`;
  }
  // Only games that are still listed count (a rescan may drop some)
  function selectedGames() {
    const games = S.list ? S.list.games : [];
    const ids = sel();
    [...ids].forEach((id) => { if (!games.some((g) => g.appid === id)) ids.delete(id); });
    return games.filter((g) => ids.has(g.appid));
  }
  function setPicked(ids, on) {
    ids.forEach((id) => (on ? sel().add(id) : sel().delete(id)));
    const top = pane.scrollTop;
    renderGames();
    pane.scrollTop = top;
  }

  function gameRow(g) {
    const where = gameWhere(g);
    const local = g.local_hash ? `<span class="time">${fmt(g.local_mtime)}<small>${esc(ago(g.local_mtime))} · ${esc(files(g.files))} · ${human(g.bytes)}</small></span>` : '<span class="note">—</span>';
    const cloud = g.cloud ? `<span class="time">${fmt(g.cloud.created_utc)}<small>${h('games.cloud_from', { device: g.cloud.device_name, n: g.versions })} · ${human(g.cloud.bytes)}</small></span>` : `<span class="note">${h('games.cloud_none')}</span>`;
    return `<tr class="${sel().has(g.appid) ? 'picked' : ''}" tabindex="0" data-card="${g.appid}" aria-label="${esc(g.name)}">
      <td class="pickcell">${pickBox(g)}</td>
      <td><div class="gcell">${art('header', g.appid, g.name, 'hdr')}<div><div class="gname">${esc(g.name)}</div><div class="gsub" title="${esc(where)}">${esc(where.split('\n')[0])}${g.roots.length > 1 ? ` (+${g.roots.length - 1})` : ''}</div>${gameFlags(g)}</div></div></td>
      <td><span class="mono">${g.appid}</span></td>
      <td>${local}</td><td>${cloud}</td>
      <td>${statusPill(g.status)}</td>
      <td><div class="acts">${gameActions(g, false)}</div></td></tr>`;
  }

  function gameCard(g) {
    const where = gameWhere(g);
    const local = g.local_hash ? `${esc(ago(g.local_mtime))} · ${human(g.bytes)}` : '—';
    const cloud = g.cloud ? `${esc(ago(g.cloud.created_utc))} · ${human(g.cloud.bytes)}` : h('games.cloud_none');
    return `<article class="gcard st-${g.status} ${sel().has(g.appid) ? 'picked' : ''}" tabindex="0" data-card="${g.appid}" aria-label="${esc(g.name)}">
      <div class="cover" data-cover="${g.appid}" title="${esc(g.name + '\n' + where)}">${art('capsule', g.appid, g.name, 'cap')}
        <div class="badges">${statusPill(g.status)}</div>${pickBox(g)}${g.steamid_bound || g.running ? `<div class="flags">${gameFlags(g)}</div>` : ''}</div>
      <div class="gbody">
        <div class="gname" title="${esc(g.name)}">${esc(g.name)}</div>
        <div class="gmeta"><span title="${h('games.col_local')}">${ic('monitor')}${local}</span><span title="${h('games.col_cloud')}">${ic('cloud')}${cloud}</span></div>
        <div class="acts">${gameActions(g, true)}</div>
      </div></article>`;
  }

  function renderGames() {
    if (S.loading && !S.list) {
      pane.innerHTML = `<div class="panehead"><h2>${h('nav.games')}</h2></div>${banners()}<div class="loading"><span class="spin"></span>${h('games.scanning')}</div>`;
      return;
    }
    const games = S.list ? S.list.games : [];
    const counts = { all: games.length };
    games.forEach((g) => { counts[g.status] = (counts[g.status] || 0) + 1; });
    const chips = ['all', 'local', 'cloud', 'conflict', 'diverged', 'new', 'sync', 'empty', 'unknown']
      .filter((k) => k === 'all' || counts[k])
      .map((k) => `<button class="chip ${S.filter === k ? 'on' : ''}" data-f="${k}" aria-pressed="${S.filter === k}">${k === 'all' ? '' : ic(STATUS_IC[k])}${h(k === 'all' ? 'games.filter_all' : 'status.' + k)}<span class="n">${counts[k] || 0}</span></button>`).join('');
    const q = S.q.trim().toLowerCase();
    const shown = games.filter((g) => (S.filter === 'all' || g.status === S.filter) && (!q || g.name.toLowerCase().includes(q) || String(g.appid).includes(q)));
    S.shown = shown;
    const grid = S.layout === 'grid';
    const nSel = selectedGames().length;
    const allShown = shown.length && shown.every((g) => sel().has(g.appid));
    const body = !shown.length
      ? `<div class="empty">${h('games.no_match')}</div>`
      : grid
        ? `<div class="ggrid ${nSel ? 'selecting' : ''}" data-navgrid="1">${shown.map(gameCard).join('')}</div>`
        : `<div class="tablewrap"><table>
            <thead><tr><th class="pickcell"><label class="pick" title="${h('games.select_shown')}"><input type="checkbox" data-pickall="1" ${allShown ? 'checked' : ''} aria-label="${h('games.select_shown')}"></label></th><th>${h('games.col_game')}</th><th>AppID</th><th>${h('games.col_local')}</th><th>${h('games.col_cloud')}</th><th>${h('games.col_status')}</th><th style="text-align:right">${h('games.col_actions')}</th></tr></thead>
            <tbody data-navgrid="1">${shown.map(gameRow).join('')}</tbody></table></div>`;
    const ready = backendReady() && !S.busy ? '' : 'disabled';
    const bulk = nSel
      ? `<button class="btn" data-act="clearSel">${h('games.clear_sel')}</button>
        <button class="btn" data-act="backupAll" ${ready}>${ic('upload')}${h('games.backup_sel', { n: nSel })}</button>
        <button class="btn primary" data-act="restoreAll" ${ready}>${ic('download')}${h('games.restore_sel', { n: nSel })}</button>`
      : `<button class="btn" data-act="backupAll" ${ready}>${ic('upload')}${h('games.backup_all')}</button>
        <button class="btn primary" data-act="restoreAll" ${ready}>${ic('download')}${h('games.restore_all')}</button>`;
    pane.innerHTML = `
      <div class="panehead"><div><div class="eyebrow">${h('nav.games')}</div><h2>${h('games.title', { device: S.info ? S.info.config.device_name : '' })}</h2></div>
        ${S.loading ? '<span class="spin"></span>' : ''}<span class="spacer"></span>
        <button class="btn" data-act="reload" ${S.loading ? 'disabled' : ''}>${ic('refresh')}${h('games.rescan')}</button>
        ${bulk}</div>
      ${banners()}
      <div class="filters">${chips}<span class="spacer"></span>
        <label class="searchbox">${ic('search')}<input type="text" id="search" placeholder="${h('games.search')}" value="${esc(S.q)}" aria-label="${h('games.search')}"></label>
        <div class="seg" role="group">
          <button class="btn icon ${grid ? 'on' : ''}" data-act="layout" data-layout="grid" title="${h('games.view_grid')}" aria-label="${h('games.view_grid')}" aria-pressed="${grid}">${ic('grid')}</button>
          <button class="btn icon ${grid ? '' : 'on'}" data-act="layout" data-layout="list" title="${h('games.view_list')}" aria-label="${h('games.view_list')}" aria-pressed="${!grid}">${ic('list')}</button>
        </div></div>
      ${body}
      <p class="note">${h('games.footer', { root: S.list ? S.list.steam_root : '' })}</p>`;
    const s = $('#search');
    if (s) s.addEventListener('input', (e) => {
      S.q = e.target.value; const pos = e.target.selectionStart; renderGames();
      const n = $('#search'); n.focus(); n.setSelectionRange(pos, pos);
    });
  }

  async function renderAccounts() {
    pane.innerHTML = `<div class="panehead"><h2>${h('nav.accounts')}</h2></div><div class="loading"><span class="spin"></span>${h('common.loading')}</div>`;
    if (!S.accounts) await loadAccounts();
    if (S.view !== 'accounts') return;
    const cards = (S.accounts || []).map((a) => {
      const games = gamesOfAccount(a.steamid64);
      return `
      <div class="card"><div class="acchead">${art('avatar', a.steamid64, a.persona_name || a.account_name, 'av')}<div class="gname">${esc(a.persona_name || a.account_name)} ${a.most_recent ? statusPill('sync', t('accounts.most_recent')) : ''}</div></div>
        <div class="k">login: ${esc(a.account_name)}</div>
        <div class="k">SteamID64: ${esc(a.steamid64)}</div>
        <div class="k">userdata/${a.account_id} · ${a.has_userdata ? h('accounts.apps', { n: a.userdata_apps }) : h('accounts.no_userdata')}</div>
        <div class="note">${a.offline_mode ? h('accounts.offline') + ' · ' : ''}${h('accounts.last_login', { time: fmt(a.timestamp * 1000) })}</div>
        <div class="note">${S.list ? (games.length ? h('accounts.bound_games', { n: games.length, games: games.map((g) => g.name).join(', ') }) : h('accounts.no_bound_games')) : ''}</div>
        ${games.length ? `<div><button class="btn sm" data-act="migrateAcct" data-sid="${esc(a.steamid64)}">${ic('swap')}${h('accounts.move_btn')}</button></div>` : ''}</div>`;
    }).join('');
    pane.innerHTML = `<div class="panehead"><h2>${h('nav.accounts')}</h2></div>
      <div class="cards">${cards || `<div class="empty">${h('accounts.none')}</div>`}</div>
      <p class="note">${h('accounts.footer')}</p>`;
  }

  async function renderHistory() {
    pane.innerHTML = `<div class="panehead"><h2>${h('history.title')}</h2></div><div class="loading"><span class="spin"></span>${h('history.loading')}</div>`;
    let items = [];
    try { items = backendReady() ? await call('history') : []; } catch (e) { fail(t('err.history_failed'), e); }
    if (S.view !== 'history') return;
    pane.innerHTML = `<div class="panehead"><h2>${h('history.title')}</h2></div>
      <div class="list">${items.map((v) => `<div><span class="mono">${fmt(v.created_utc)}</span><span>${esc(v.game_name)} <span class="note">· ${esc(files(v.files))} · ${human(v.bytes)}</span>${v.kept ? `<span class="kept">${ic('key')}${h('history.kept', { device: v.kept.by_device, time: fmt(v.kept.at_utc) })}</span>` : ''}</span><span class="note">${esc(v.device_name)} (${v.os})${v.kept ? `<button class="btn sm" data-act="unkeep" data-id="${v.appid}" data-ver="${esc(v.id)}" title="${h('history.unkeep_hint')}">${h('history.unkeep')}</button>` : ''}</span></div>`).join('') || `<div class="empty" style="display:block">${h('history.empty')}</div>`}</div>
      <p class="note">${h('history.footer', { n: S.info ? S.info.config.keep_versions : 10 })} ${h('history.kept_footer')}</p>`;
  }

  function renderSettings() {
    const c = S.info.config, b = c.backend;
    const langOpt = (v, label) => `<option value="${v}" ${c.language === v ? 'selected' : ''}>${esc(label)}</option>`;
    pane.innerHTML = `<div class="panehead"><h2>${h('nav.settings')}</h2></div>
    <div class="form">
      <div class="group"><h3>${h('settings.this_pc')}</h3>
        <div class="field"><label class="l" for="lang">${h('settings.language')}</label>
          <select id="lang">${langOpt('auto', t('settings.language_auto'))}${langOpt('en', 'English')}${langOpt('vi', 'Tiếng Việt')}</select></div>
        <div class="field"><label class="l" for="devName">${h('settings.device_name')}</label>
          <input type="text" id="devName" value="${esc(c.device_name)}"><span class="note">${h('settings.device_name_hint', { id: c.device_id })}</span></div>
        <div class="field"><label class="l" for="steamRoot">${h('settings.steam_folder')}</label>
          <input type="text" id="steamRoot" value="${esc(c.steam_root_override || '')}" placeholder="${esc(S.info.steam_root || t('settings.steam_not_found'))}"><span class="note">${h('settings.steam_folder_hint')}</span></div>
        <div class="field"><span class="l">${h('settings.steam_library')}</span>
          <div class="row"><button class="btn" data-act="addSteam" ${S.info.steam_root ? '' : 'disabled'}>${ic('library')}${h('settings.add_to_steam')}</button></div>
          <span class="note">${h('settings.add_to_steam_hint')}</span></div>
      </div>
      <div class="group"><h3>${h('settings.storage')}</h3>
        <div class="field"><label class="l" for="bk">${h('settings.storage_kind')}</label>
          <select id="bk">
            <option value="none" ${b.kind === 'none' ? 'selected' : ''}>${h('settings.storage_none')}</option>
            <option value="rclone" ${b.kind === 'rclone' ? 'selected' : ''}>${h('settings.storage_rclone')}</option>
            <option value="folder" ${b.kind === 'folder' ? 'selected' : ''}>${h('settings.storage_folder')}</option>
          </select></div>
        <div id="bkRclone" ${b.kind === 'folder' ? 'hidden' : ''} class="field">
          <div id="rcPanel" class="steps"><div class="loading"><span class="spin"></span>${h('rclone.checking')}</div></div>
          <details class="adv"><summary>${h('settings.advanced')}</summary>
            <div class="field"><label class="l" for="rRemote">${h('settings.remote')}</label>
              <input type="text" id="rRemote" value="${esc(b.kind === 'rclone' ? b.remote : 'gdrive:Checkpoint')}"></div>
            <div class="field"><label class="l" for="rBin">${h('settings.rclone_path')}</label>
              <input type="text" id="rBin" value="${esc(b.kind === 'rclone' ? (b.bin || '') : '')}" placeholder="${h('settings.rclone_path_hint')}"></div>
          </details>
        </div>
        <div id="bkFolder" ${b.kind === 'folder' ? '' : 'hidden'} class="field">
          <label class="l" for="fPath">${h('settings.folder')}</label>
          <div class="row"><input type="text" id="fPath" value="${esc(b.kind === 'folder' ? b.path : '')}" placeholder="G:\\My Drive\\Checkpoint"><button class="btn" data-act="pick">${ic('folder')}${h('settings.choose')}</button></div>
        </div>
        ${backendReady() ? `<div class="field danger-zone"><span class="l">${h('settings.delete_all')}</span>
          <span class="note">${h('settings.delete_all_hint')}</span>
          <div class="row"><button class="btn" data-act="delAll" ${S.busy ? 'disabled' : ''}>${ic('trash')}${h('settings.delete_all_btn')}</button></div></div>` : ''}
      </div>
      <div class="group"><h3>${h('settings.backup')}</h3>
        <label class="check" for="incCfg"><input type="checkbox" id="incCfg" ${c.include_config ? 'checked' : ''}> <span>${h('settings.include_config')}<br><span class="note">${h('settings.include_config_hint')}</span></span></label>
        <div class="field"><label class="l" for="onConflict">${h('settings.launch_conflict')}</label>
          <select id="onConflict">${['ask', 'play', 'stop'].map((v) => `<option value="${v}" ${(c.launch_conflict || 'ask') === v ? 'selected' : ''}>${esc(t({ ask: 'settings.launch_conflict_ask', play: 'settings.launch_conflict_play', stop: 'settings.launch_conflict_stop' }[v]))}</option>`).join('')}</select>
          <span class="note">${h('settings.launch_conflict_hint')}</span></div>
        <label class="check" for="chkNotify"><input type="checkbox" id="chkNotify" ${c.launch_notifications !== false ? 'checked' : ''}> <span>${h('settings.launch_notifications')}<br><span class="note">${h('settings.launch_notifications_hint')}</span></span></label>
        <div class="field"><label class="l" for="keep">${h('settings.keep_versions')}</label><input type="number" id="keep" min="1" max="100" value="${c.keep_versions}" style="width:100px"></div>
      </div>
      <div class="group"><h3>${h('settings.manifest')}</h3>
        <p class="note">${S.info.manifest_games ? h('settings.manifest_info', { n: S.info.manifest_games.toLocaleString(locale()), time: fmt(S.info.manifest_updated) }) : h('settings.manifest_missing')} ${h('settings.manifest_source')}</p>
        <div class="row"><button class="btn" data-act="manifest">${ic('download')}${h('settings.manifest_update')}</button></div>
      </div>
      <div class="group"><h3>${h('settings.updates')}</h3>
        <p class="note">${h('settings.version', { version: S.info.version })}</p>
        <label class="check" for="chkUpd"><input type="checkbox" id="chkUpd" ${c.check_updates !== false ? 'checked' : ''}> <span>${h('settings.check_updates')}<br><span class="note">${h('settings.check_updates_hint')}</span></span></label>
        <label class="check" for="chkBeta"><input type="checkbox" id="chkBeta" ${c.beta_updates ? 'checked' : ''}> <span>${h('settings.beta_updates')}<br><span class="note">${h('settings.beta_updates_hint')}</span></span></label>
        <div class="row"><button class="btn" data-act="checkUpd">${ic('refresh')}${h('settings.check_now')}</button></div>
      </div>
      <div class="row"><button class="btn primary" data-act="saveCfg">${ic('save')}${h('settings.save')}</button><span class="note">${h('settings.saved_in', { path: S.info.config_dir })}</span></div>
    </div>`;
    $('#bk').addEventListener('change', (e) => {
      $('#bkRclone').hidden = e.target.value === 'folder';
      $('#bkFolder').hidden = e.target.value !== 'folder';
    });
    loadRclone();
  }

  // ---------- accounts ----------
  const SID_BASE = 76561197960265728n;
  function toSid(id) {
    try { const n = BigInt(id); return (n >= SID_BASE ? n : n + SID_BASE).toString(); } catch (e) { return String(id); }
  }
  function acctName(sid) {
    const a = (S.accounts || []).find((x) => x.steamid64 === sid);
    return a ? (a.persona_name || a.account_name) + (a.account_name && a.persona_name ? ` (${a.account_name})` : '') : `SteamID …${String(sid).slice(-6)}`;
  }
  function gamesOfAccount(sid) {
    return (S.list ? S.list.games : []).filter((g) => (g.steam_ids || []).some((x) => toSid(x) === sid));
  }

  // ---------- move a save to another account ----------
  let W = null;
  const CLASS_PILL = { none: 's-new', folder: 's-sync', in_file: 's-local', opaque: 's-local' };

  function openMigrate(opts) {
    if (!S.list) { toast(t('move.wait_scan'), true); return; }
    const pool = opts.appid ? [game(opts.appid)] : S.list.games;
    const sids = [...new Set(pool.flatMap((g) => (g.steam_ids || []).map(toSid)))];
    if (!sids.length) { toast(t('move.no_bound_saves'), true); return; }
    const from = opts.from && sids.includes(opts.from) ? opts.from : sids[0];
    W = { step: 1, sids, from, to: null, only: opts.appid || null, picked: null, previews: [], results: [], errors: [] };
    renderWizard();
  }

  function wizSteps() {
    return `<div class="wsteps">${['move.step_choose', 'move.step_prepare', 'move.step_test'].map((k, i) => `<span class="${W.step === i + 1 ? 'on' : ''}">${i + 1} · ${h(k)}</span>`).join('')}</div>`;
  }

  function renderWizard() {
    if (!W) return;
    if (W.step === 1) return wizardStep1();
    if (W.step === 2) return wizardStep2();
    return wizardStep3();
  }

  function wizardStep1() {
    const targets = (S.accounts || []).filter((a) => a.steamid64 !== W.from);
    if (!W.to || W.to === W.from || !targets.some((a) => a.steamid64 === W.to)) {
      const cur = targets.find((a) => a.most_recent);
      W.to = (cur || targets[0] || {}).steamid64 || null;
    }
    const games = gamesOfAccount(W.from).filter((g) => !W.only || g.appid === W.only);
    if (!W.picked) W.picked = new Set(games.map((g) => g.appid));
    const fromOpts = W.sids.map((s) => `<option value="${esc(s)}" ${s === W.from ? 'selected' : ''}>${esc(acctName(s))} · ${esc(s)}</option>`).join('');
    const toOpts = targets.map((a) => `<option value="${esc(a.steamid64)}" ${a.steamid64 === W.to ? 'selected' : ''}>${esc(acctName(a.steamid64))}${a.most_recent ? ' · ' + h('common.signed_in') : ''}</option>`).join('');
    const list = games.map((g) => `<label class="grow" for="wg${g.appid}"><input type="checkbox" id="wg${g.appid}" data-wg="${g.appid}" ${W.picked.has(g.appid) ? 'checked' : ''}>
        <span><b>${esc(g.name)}</b><br><span class="note">${esc(files(g.files))} · ${human(g.bytes)} · ${g.cloud ? h('move.last_backup', { time: fmt(g.cloud.created_utc) }) : h('move.no_cloud_backup')}</span></span></label>`).join('');
    modal(`<h3>${h('move.title')}</h3>${wizSteps()}
      <div class="acctpick">
        <div class="field"><label class="l" for="wFrom">${h('move.from')}</label><select id="wFrom">${fromOpts}</select></div>
        <span class="arrow" aria-hidden="true">→</span>
        <div class="field"><label class="l" for="wTo">${h('move.to')}</label>${targets.length ? `<select id="wTo">${toOpts}</select>` : `<span class="note">${h('move.no_target')}</span>`}</div>
      </div>
      <div class="glist">${list || `<div class="empty">${h('move.no_games')}</div>`}</div>
      <p class="note">${h('move.copy_only')}</p>
      <div class="mfoot"><button class="btn" data-close="1">${h('common.cancel')}</button><button class="btn primary" data-act="wzNext" ${targets.length && games.length ? '' : 'disabled'}>${h('common.next')}</button></div>`);
    $('#wFrom').addEventListener('change', (e) => { W.from = e.target.value; W.picked = null; wizardStep1(); });
    const sel = $('#wTo'); if (sel) sel.addEventListener('change', (e) => { W.to = e.target.value; });
    layer.querySelectorAll('[data-wg]').forEach((c) => c.addEventListener('change', () => {
      const id = +c.dataset.wg; if (c.checked) W.picked.add(id); else W.picked.delete(id);
    }));
  }

  async function wizardPrepare() {
    const ids = [...W.picked];
    if (!ids.length) { toast(t('move.pick_one'), true); return; }
    busyModal(t('move.title'));
    W.previews = [];
    for (const appid of ids) {
      try { W.previews.push(await call('migrate_preview', { appid, from: W.from, to: W.to })); }
      catch (e) { W.previews.push({ appid, name: game(appid).name, error: String(e) }); }
    }
    W.step = 2; renderWizard();
  }

  function blockedReason(p) {
    if (p.error) return p.error;
    if (p.running) return t('move.blocked_running');
    if (p.pending_id) return t('move.blocked_pending');
    if (p.class === 'none') return t('move.blocked_none');
    return null;
  }

  function wizardStep2() {
    const to = acctName(W.to);
    const cards = W.previews.map((p) => {
      const blocked = blockedReason(p);
      const g = game(p.appid);
      const ck = (ok, html) => `<div><span class="ck ${ok ? 'ok' : 'todo'}">${ok ? '✓' : '!'}</span><span>${html}</span></div>`;
      const checks = blocked ? `<div class="alert bad">${esc(blocked)}</div>` : `<div class="checklist">
          ${ck(true, h('move.check_not_running'))}
          ${g && g.cloud ? ck(true, h('move.check_backup', { time: fmt(g.cloud.created_utc) })) : ck(false, h('move.check_no_backup'))}
          ${p.target_dir_exists ? ck(true, h('move.check_target_ok', { account: to })) : ck(false, h('move.check_target_new', { account: to }))}
          ${p.overwrite ? ck(false, h('move.check_overwrite', { account: to, n: p.overwrite })) : ''}
          ${p.class === 'opaque' ? ck(false, h('move.check_opaque', { entropy: p.max_entropy.toFixed(3) })) : ''}
          ${p.class === 'in_file' ? `<label class="check" for="patch${p.appid}"><input type="checkbox" id="patch${p.appid}" checked> <span>${h('move.patch', { n: p.files.filter((f) => f.id_hits).length })}</span></label>` : ''}
        </div>`;
      const f = p.files && p.files[0];
      const map = f ? `<div class="pathmap"><span class="del">  ${esc(f.from)}</span>\n<span class="add">→ ${esc(f.to)}</span>${p.files.length > 1 ? `\n<span class="del">  ${h('move.more_files', { n: p.files.length - 1 })}</span>` : ''}</div>` : '';
      const pill = p.class ? `<span class="pill ${CLASS_PILL[p.class] || ''}">${h('move.class_' + p.class)}</span>` : '';
      return `<div class="step ${blocked ? 'todo' : 'done'}"><div class="row"><b>${esc(p.name)}</b><span class="spacer"></span>${pill}</div>${checks}${map}</div>`;
    }).join('');
    const ready = W.previews.filter((p) => !blockedReason(p)).length;
    modal(`<h3>${h('move.title_pair', { from: acctName(W.from), to })}</h3>${wizSteps()}
      <div class="steps">${cards}</div>
      <div class="mfoot"><button class="btn" data-act="wzBack">${h('common.back')}</button><button class="btn primary" data-act="wzApply" ${ready ? '' : 'disabled'}>${h('move.apply_btn', { n: ready })}</button></div>`);
  }

  async function wizardApply() {
    const todo = W.previews.filter((p) => !blockedReason(p));
    const patch = {};
    todo.forEach((p) => { const c = $('#patch' + p.appid); patch[p.appid] = !!(c && c.checked); });
    busyModal(t('move.applying'));
    S.busy = true;
    W.results = []; W.errors = [];
    for (const p of todo) {
      const el = $('#busyText'); if (el) el.textContent = p.name;
      try { W.results.push(await call('migrate_apply', { appid: p.appid, from: W.from, to: W.to, patchInFile: patch[p.appid] })); }
      catch (e) { W.errors.push(`${p.name}: ${e}`); }
    }
    S.busy = false;
    W.step = 3; W.done = {};
    renderWizard();
    loadGames();
  }

  function wizardStep3() {
    const from = acctName(W.from), to = acctName(W.to);
    const rows = W.results.map((m) => {
      const d = W.done[m.id];
      const right = d === 'keep' ? `<span class="pill s-sync">${h('move.kept')}</span>`
        : d === 'undo' ? `<span class="pill s-new">${h('move.undone')}</span>`
        : `<button class="btn sm" data-act="migUndo" data-mid="${esc(m.id)}">${h('move.undo_btn')}</button><button class="btn sm primary" data-act="migKeep" data-mid="${esc(m.id)}">${h('move.keep_btn')}</button>`;
      return `<div class="row mrow2"><span><b>${esc(m.game_name)}</b><br><span class="note">${esc(files(m.files))}${m.patched ? ' · ' + h('move.patched', { n: m.patched }) : ''}</span></span><span class="spacer"></span>${right}</div>`;
    }).join('');
    const pending = W.results.some((m) => !W.done[m.id]);
    modal(`<h3>${h('move.step_test')}</h3>${wizSteps()}
      ${W.results.length ? `<div class="alert">${h('move.test_intro', { account: to })}</div>` : ''}
      ${W.errors.map((e) => `<div class="alert bad">${esc(e)}</div>`).join('')}
      <div class="glist">${rows}</div>
      <p class="note">${h('move.test_note', { from, to })}</p>
      <div class="mfoot"><button class="btn ${pending ? '' : 'primary'}" data-close="1">${pending ? h('move.close_to_test') : h('common.done')}</button></div>`);
  }

  async function migDecide(kind, id) {
    const inWizard = W && W.step === 3 && W.results.some((m) => m.id === id);
    if (!inWizard) busyModal(kind === 'keep' ? t('move.keeping') : t('move.undoing'));
    try {
      if (kind === 'keep') {
        const r = await call('migrate_keep', { id });
        toast(t('move.kept_toast', { game: r.migration.game_name }));
        if (r.warning) toast(r.warning, true);
      } else {
        const m = await call('migrate_undo', { id });
        toast(t('move.undone_toast', { game: m.game_name }));
      }
      if (inWizard) { W.done[id] = kind; renderWizard(); } else close();
    } catch (e) { fail(kind === 'keep' ? t('err.keep_failed') : t('err.undo_failed'), e); }
    loadGames();
  }

  // ---------- rclone / Google Drive ----------
  const OS_NAME = { windows: 'Windows', linux: 'Linux', macos: 'macOS' };

  async function loadRclone() {
    let st;
    try { st = await call('rclone_status'); } catch (e) { st = null; }
    const panel = $('#rcPanel');
    if (!panel || S.view !== 'settings') return;
    if (!st) { panel.innerHTML = `<p class="note">${h('rclone.check_failed')}</p>`; return; }
    const r = st.remote;
    const step1 = st.found
      ? `<div class="step done"><b>1. rclone</b><span>${h(st.managed ? 'rclone.found_managed' : 'rclone.found', { version: st.version })}</span><span class="note mono">${esc(st.bin)}</span></div>`
      : `<div class="step todo"><b>1. rclone</b><span>${h('rclone.missing')}</span>
           <div class="row"><button class="btn primary" data-act="rinstall" ${st.platform ? '' : 'disabled'}>${ic('download')}${h('rclone.install')}</button>
           <span class="note">${h('rclone.install_hint', { platform: st.platform || '' })}</span></div></div>`;
    let remoteLine;
    if (!r) remoteLine = h('rclone.not_connected');
    else if (r.custom_client) remoteLine = h('rclone.connected', { name: r.name, scope: r.scope || '?' });
    else remoteLine = h('rclone.shared_client', { name: r.name });
    const ok = r && r.custom_client;
    // The saved values, so it is clear which client this PC uses. Hidden until the eye button is pressed.
    let cred = null;
    if (ok) { try { cred = await call('rclone_client'); } catch (e) { cred = null; } }
    if (!$('#rcPanel') || S.view !== 'settings') return;
    const secretField = (id, label, ph, val) => `<div class="field"><label class="l" for="${id}">${label}</label>
        <div class="secret"><input type="password" id="${id}" placeholder="${ph}" value="${esc(val || '')}" autocomplete="off" spellcheck="false" ${st.found ? '' : 'disabled'}>
        <button class="btn icon" type="button" data-act="reveal" data-for="${id}" title="${h('rclone.show')}" aria-label="${h('rclone.show')}" aria-pressed="false" ${st.found ? '' : 'disabled'}>${ic('eye')}</button></div></div>`;
    const step2 = `<div class="step ${ok ? 'done' : 'todo'}"><b>2. Google Drive</b><span>${remoteLine}</span>
        ${secretField('cid', 'Client ID', 'xxxx.apps.googleusercontent.com', cred && cred.client_id)}
        ${secretField('csec', 'Client secret', 'GOCSPX-…', cred && cred.client_secret)}
        ${cred ? `<span class="note">${h('rclone.saved_values')}</span>` : ''}
        <div class="row"><button class="btn ${ok ? '' : 'primary'}" data-act="rconnect" ${st.found ? '' : 'disabled'}>${ic('cloud')}${h(r ? 'rclone.reconnect' : 'rclone.connect')}</button>
        <span class="note">${h('rclone.connect_hint')}</span></div></div>`;
    panel.innerHTML = `<p class="note">${h('rclone.this_pc', { os: OS_NAME[st.os] || st.os, arch: st.arch })}</p>${step1}${step2}`;
  }

  function readSettings() {
    const kind = $('#bk').value;
    let backend = { kind: 'none' };
    if (kind === 'rclone') backend = { kind, remote: $('#rRemote').value.trim(), bin: $('#rBin').value.trim() || null };
    if (kind === 'folder') backend = { kind, path: $('#fPath').value.trim() };
    return {
      ...S.info.config,
      device_name: $('#devName').value.trim() || S.info.config.device_name,
      steam_root_override: $('#steamRoot').value.trim() || null,
      backend,
      include_config: $('#incCfg').checked,
      launch_notifications: $('#chkNotify').checked,
      launch_conflict: $('#onConflict').value,
      keep_versions: Math.max(1, Math.min(100, parseInt($('#keep').value, 10) || 10)),
      language: $('#lang').value,
      check_updates: $('#chkUpd').checked,
      beta_updates: $('#chkBeta').checked,
    };
  }

  // ---------- backup / restore ----------
  const game = (id) => S.list.games.find((g) => g.appid === id);
  const opName = (kind) => t(kind === 'backup' ? 'common.backup' : 'common.restore');

  function box(title, ms, sub, newer) {
    return `<div class="box ${newer ? 'newer' : ''}"><span class="lbl">${esc(title)}</span><span class="big">${fmt(ms)}</span><span class="note">${esc(sub)}</span></div>`;
  }

  async function doOp(kind, g, opts) {
    const pg = progressModal(`${opName(kind)} · ${g.name}`);
    S.busy = true;
    try {
      const r = kind === 'backup'
        ? await call('backup_game', { appid: g.appid, force: !!opts.force, onProgress: pg.ch })
        : await call('restore_game', { appid: g.appid, versionId: opts.versionId || null, force: !!opts.force, remapFrom: opts.remapFrom || null, remapTo: opts.remapTo || null, onProgress: pg.ch });
      // Let the finished list show for a moment before the dialog closes
      if ($('#pgList') && !$('#pgList').hidden) { pg.done(); await new Promise((ok) => setTimeout(ok, 700)); }
      close();
      // A restore over a local save kept it as a snapshot: offer to go back to it right away
      const undo = kind === 'restore' && r.outcome === 'restored' && g.local_hash
        ? { label: t('snap.undo'), run: () => openSnapshots(g.appid, 'newest') } : null;
      toast(r.message, false, undo);
      S.busy = false;
      await loadGames();
    } catch (e) {
      S.busy = false;
      const msg = String(e);
      if (msg.startsWith('NEEDS_CONFIRM')) { close(); return kind === 'backup' ? confirmBackup(g) : openRestore(g); }
      fail(t('err.op_failed', { op: opName(kind), game: g.name }), msg); render();
    }
  }

  function startBackup(g) {
    if (['cloud', 'conflict', 'diverged'].includes(g.status)) return confirmBackup(g);
    doOp('backup', g, {});
  }

  function confirmBackup(g) {
    const c = g.cloud;
    const text = g.status === 'conflict' ? h('backup.warn_conflict', { device: c.device_name })
      : g.status === 'diverged' ? h('backup.warn_diverged')
        : h('backup.warn_cloud_newer', { device: c.device_name });
    modal(`<h3>${h('common.backup')} · ${esc(g.name)}</h3>
      <div class="cmp">${box(t('common.local_this_pc'), g.local_mtime, ago(g.local_mtime), g.local_mtime > c.created_utc)}${box(t('common.cloud_from', { device: c.device_name }), c.created_utc, ago(c.created_utc), c.created_utc >= g.local_mtime)}</div>
      <div class="alert ${g.status === 'conflict' ? 'bad' : 'warn'}"><span>${text}</span><span>${h('backup.warn_tail')}</span></div>
      <div class="mfoot"><button class="btn" data-close="1">${h('common.cancel')}</button>
        <button class="btn primary" data-act="toRestore" data-id="${g.appid}">${h('backup.see_cloud')}</button>
        <button class="btn danger" data-act="forceBackup" data-id="${g.appid}">${h('backup.force')}</button></div>`);
  }

  async function openRestore(g) {
    busyModal(`${t('common.restore')} · ${g.name}`);
    let versions = [];
    try { versions = await call('list_versions', { appid: g.appid }); } catch (e) { return fail(t('err.versions_failed'), e); }
    if (!versions.length) { close(); return toast(t('restore.no_versions'), true); }
    renderRestore(g, versions, versions[0].id);
  }

  function renderRestore(g, versions, selId) {
    const v = versions.find((x) => x.id === selId) || versions[0];
    const latest = versions[0];
    const hasLocal = !!g.local_hash;
    const same = hasLocal && g.local_hash === v.content_hash;
    const localNewer = hasLocal && g.local_mtime > v.created_utc;
    let alert = '';
    if (same) alert = `<div class="alert">${h('restore.same')}</div>`;
    else if (!hasLocal) alert = `<div class="alert">${h('restore.no_local')}</div>`;
    else if (g.status === 'conflict') alert = `<div class="alert bad">${h('restore.conflict', { device: latest.device_name })}</div>`;
    else if (g.status === 'local' || localNewer) alert = `<div class="alert warn">${h('restore.local_newer', { diff: diff(g.local_mtime - v.created_utc) })}</div>`;
    else if (g.status === 'diverged') alert = `<div class="alert warn">${h('restore.diverged', { diff: diff(v.created_utc - g.local_mtime) })}</div>`;
    else alert = `<div class="alert">${h('restore.cloud_newer', { device: v.device_name })}</div>`;
    if (v.id !== latest.id) alert += `<div class="alert warn">${h('restore.older_version', { time: fmt(latest.created_utc) })}</div>`;
    // A backup tied to exactly one account: allow writing it into another account
    const verAccts = [...new Set((v.steam_ids || []).map(toSid))];
    let acctPick = '';
    if (verAccts.length === 1) {
      const src = verAccts[0];
      const alias = S.list && S.list.account_aliases ? S.list.account_aliases[src] : null;
      const cur = (S.accounts || []).find((a) => a.most_recent);
      const def = alias || src;
      const others = (S.accounts || []).filter((a) => a.steamid64 !== src);
      const opt = (sid, label) => `<option value="${esc(sid)}" ${sid === def ? 'selected' : ''}>${esc(label)}</option>`;
      const note = alias ? h('restore.account_alias', { from: acctName(src), to: acctName(alias) })
        : cur && cur.steamid64 !== src ? h('restore.account_other', { from: acctName(src), current: acctName(cur.steamid64) })
          : h('restore.account_same');
      acctPick = `<div class="field"><label class="l" for="remapTo">${h('restore.account')}</label>
        <select id="remapTo" data-from="${esc(src)}">${opt(src, t('restore.account_original', { account: acctName(src) }))}${others.map((a) => opt(a.steamid64, acctName(a.steamid64) + (a.most_recent ? ' · ' + t('common.signed_in') : ''))).join('')}</select>
        <span class="note">${note}</span></div>`;
    } else if (v.steam_ids && v.steam_ids.length) {
      alert += `<div class="alert">${h('restore.bound_many', { ids: v.steam_ids.join(', ') })}</div>`;
    }
    if (S.info.os === 'linux' && v.os === 'windows') alert += `<div class="alert">${h('restore.proton_note')}</div>`;
    const danger = hasLocal && !same && (g.status === 'local' || g.status === 'conflict' || g.status === 'diverged' || localNewer);
    const opts = versions.map((x) => `<option value="${esc(x.id)}" ${x.id === v.id ? 'selected' : ''}>${fmt(x.created_utc)} · ${esc(x.device_name)} · ${esc(files(x.files))} · ${human(x.bytes)}${x.kept ? ` · ${esc(t('restore.kept'))}` : ''}</option>`).join('');
    modal(`<h3>${h('common.restore')} · ${esc(g.name)}</h3>
      <div class="field"><label class="l" for="ver">${h('restore.version')}</label><select id="ver">${opts}</select></div>
      <div class="cmp">${box(t('common.local_this_pc'), g.local_mtime, hasLocal ? `${ago(g.local_mtime)} · ${files(g.files)}` : t('restore.no_save'), localNewer)}${box(t('common.cloud_from', { device: v.device_name }), v.created_utc, `${ago(v.created_utc)} · ${v.os}`, !localNewer && !same && hasLocal)}</div>
      ${alert}
      ${acctPick}
      ${hasLocal && !same ? `<p class="note">${h('restore.snapshot_note')}</p>` : ''}
      <div class="mfoot"><button class="btn" data-close="1">${h('common.cancel')}</button>
        ${danger ? `<button class="btn primary" data-act="toBackup" data-id="${g.appid}">${h('restore.backup_instead')}</button>` : ''}
        <button class="btn ${danger ? 'danger' : 'primary'}" data-act="doRestore" data-id="${g.appid}" data-ver="${esc(v.id)}" ${same ? 'disabled' : ''}>${danger ? h('restore.force') : h('common.restore')}</button></div>`);
    $('#ver').addEventListener('change', (e) => renderRestore(g, versions, e.target.value));
    const rt = $('#remapTo');
    const syncBtn = () => {
      const b = layer.querySelector('[data-act="doRestore"]');
      if (b) b.disabled = same && (!rt || rt.value === rt.dataset.from);
    };
    if (rt) { rt.addEventListener('change', syncBtn); syncBtn(); }
  }

  // What Backup all / Restore all would do with each game. `run`: can run without a decision.
  function planFor(kind, games) {
    return games.map((g) => {
      let act, cls, run = false;
      if (g.running) { act = 'all.skip_running'; cls = 's-conflict'; }
      else if (kind === 'backup') {
        if (g.status === 'local' || g.status === 'new') { act = 'common.backup'; cls = 's-local'; run = true; }
        else if (g.status === 'sync') { act = 'all.skip_same'; cls = 's-sync'; }
        else if (['cloud', 'conflict', 'diverged'].includes(g.status)) { act = 'all.skip_manual'; cls = 's-conflict'; }
        else { act = 'all.skip_no_save'; cls = 's-new'; }
      } else {
        if (g.status === 'cloud') { act = 'common.restore'; cls = 's-cloud'; run = true; }
        else if (g.status === 'sync') { act = 'all.skip_same'; cls = 's-sync'; }
        else if (['local', 'conflict', 'diverged'].includes(g.status)) { act = 'all.skip_manual'; cls = 's-conflict'; }
        else { act = 'all.skip_no_cloud'; cls = 's-new'; }
      }
      // Backup uploads the local save; restore downloads the latest cloud version
      const bytes = kind === 'backup' ? (g.local_hash ? g.bytes : 0) : (g.cloud ? g.cloud.bytes : 0);
      return { g, act, cls, run, bytes };
    });
  }

  let P = null; // the open Backup / Restore plan
  function openAll(kind) {
    if (!S.list) return;
    const picked = selectedGames();
    const games = picked.length ? picked : S.list.games;
    const plan = planFor(kind, games);
    P = { kind, plan, on: new Set(plan.filter((p) => p.run).map((p) => p.g.appid)), only: picked.length > 0 };
    renderPlan();
  }

  function renderPlan() {
    const runnable = P.plan.filter((p) => p.run);
    const chosen = runnable.filter((p) => P.on.has(p.g.appid));
    const total = chosen.reduce((n, p) => n + p.bytes, 0);
    const title = P.only ? h(P.kind === 'backup' ? 'games.backup_sel' : 'games.restore_sel', { n: P.plan.length }) : h(P.kind === 'backup' ? 'games.backup_all' : 'games.restore_all');
    const rows = P.plan.map((p) => {
      const id = `pl${p.g.appid}`;
      const box = p.run
        ? `<input type="checkbox" id="${id}" data-plan="${p.g.appid}" ${P.on.has(p.g.appid) ? 'checked' : ''}>`
        : '<input type="checkbox" disabled>';
      return `<label class="prow ${p.run ? '' : 'off'}" for="${id}">${box}<span class="pname">${esc(p.g.name)}</span>
        <span class="note psize">${p.bytes ? human(p.bytes) : '—'}</span><span class="pill ${p.cls}">${h(p.act)}</span></label>`;
    }).join('');
    modal(`<h3>${title}</h3>
      <p class="note">${h('all.note')}</p>
      ${runnable.length > 1 ? `<div class="row"><button class="btn sm" data-act="planAll">${h('all.select_all')}</button><button class="btn sm" data-act="planNone">${h('all.select_none')}</button></div>` : ''}
      <div class="plan">${rows}</div>
      <div class="mfoot"><button class="btn" data-close="1">${h('common.cancel')}</button><button class="btn primary" data-act="runAll" ${chosen.length ? '' : 'disabled'}>${h('all.run_size', { n: chosen.length, size: human(total) })}</button></div>`);
    layer.querySelectorAll('[data-plan]').forEach((c) => c.addEventListener('change', () => {
      const id = +c.dataset.plan;
      if (c.checked) P.on.add(id); else P.on.delete(id);
      refreshPlanFoot();
    }));
  }

  // Update the Run button without rebuilding the list (keeps focus and scroll)
  function refreshPlanFoot() {
    const chosen = P.plan.filter((p) => p.run && P.on.has(p.g.appid));
    const b = layer.querySelector('[data-act="runAll"]');
    if (!b) return;
    b.disabled = !chosen.length;
    b.textContent = t('all.run_size', { n: chosen.length, size: human(chosen.reduce((n, p) => n + p.bytes, 0)) });
  }

  async function runAll() {
    const kind = P.kind;
    const todo = P.plan.filter((p) => p.run && P.on.has(p.g.appid)).map((p) => p.g);
    const label = t(kind === 'backup' ? 'games.backup_all' : 'games.restore_all');
    S.busy = true;
    let ok = 0; const errs = [];
    for (let i = 0; i < todo.length; i++) {
      const g = todo[i];
      const pg = progressModal(label, `${i + 1}/${todo.length} · ${g.name}`);
      try {
        if (kind === 'backup') await call('backup_game', { appid: g.appid, force: false, onProgress: pg.ch });
        else await call('restore_game', { appid: g.appid, versionId: null, force: false, onProgress: pg.ch });
        ok++;
      } catch (e) { errs.push(`${g.name}: ${String(e).startsWith('NEEDS_CONFIRM') ? t('all.changed_skipped') : e}`); }
    }
    S.busy = false; close();
    toast(t('all.done', { ok, n: todo.length }));
    if (errs.length) fail(t('err.all_failed', { op: label, n: errs.length }), errs.join('\n'));
    if (P.only && !errs.length) sel().clear();
    P = null;
    await loadGames();
  }

  // ---------- one game: details, delete cloud backup, report ----------
  const storageName = () => {
    const b = S.info && S.info.config.backend;
    return !b ? '' : b.kind === 'folder' ? b.path : b.kind === 'rclone' ? b.remote : '';
  };

  function openGame(g) {
    const where = g.roots.length ? g.roots.map((r) => `<div class="mono">${esc(r)}</div>`).join('') : `<span class="note">${esc(gameWhere(g))}</span>`;
    const local = g.local_hash ? `${fmt(g.local_mtime)} · ${esc(files(g.files))} · ${human(g.bytes)}` : h('game.no_local');
    const cloud = g.cloud ? h('game.cloud_info', { n: g.versions, time: fmt(g.cloud.created_utc), device: g.cloud.device_name }) : h('games.cloud_none');
    const canDelete = g.cloud && backendReady() && !S.busy;
    modal(`<div class="ghead">${art('header', g.appid, g.name, 'hdr')}<div><h3>${esc(g.name)}</h3><div class="note">AppID <span class="mono">${g.appid}</span> · ${statusPill(g.status)}</div></div></div>
      <div class="kv">
        <span class="l">${h('game.save_folders')}</span><div>${where}</div>
        <span class="l">${h('games.col_local')}</span><div>${local}</div>
        <span class="l">${h('games.col_cloud')}</span><div>${cloud}</div>
        <span class="l">${h('snap.row')}</span><div class="row">${g.snapshots
          ? `<span>${h('snap.count', { n: g.snapshots })}</span><button class="btn sm" data-act="snaps" data-id="${g.appid}">${ic('history')}${h('snap.view')}</button>`
          : `<span class="note">${h('snap.none')}</span>`}</div>
      </div>
      ${g.steamid_bound || g.running ? `<div>${gameFlags(g)}</div>` : ''}
      ${S.info && S.info.launch_prefix ? `<div class="field"><label class="l" for="launchOpt">${h('game.launch_option')}</label>
        <div class="secret"><input type="text" id="launchOpt" readonly value="${esc(`${S.info.launch_prefix} ${g.appid} -- %command%`)}">
        <button class="btn" data-act="copyLaunch">${ic('copy')}${h('game.copy')}</button></div>
        <span class="note">${h('game.launch_hint')}</span></div>` : ''}
      <div class="mfoot"><button class="btn" data-act="delCloud" data-id="${g.appid}" ${canDelete ? '' : 'disabled'}>${ic('trash')}${h('game.delete_cloud')}</button>
        <button class="btn" data-act="reportGame" data-id="${g.appid}">${ic('bug')}${h('game.report')}</button><span class="spacer"></span>
        <button class="btn primary" data-close="1" data-autofocus>${h('common.close')}</button></div>`);
  }

  /** A game's actions as one menu, for the controller (A on a game) and Enter on the keyboard. */
  function openGameMenu(id) {
    const g = game(id);
    if (!g) return;
    const dis = !backendReady() || g.running || S.busy;
    const item = (act, icon, label, hint, off) => `<button class="btn gm" data-act="${act}" data-id="${g.appid}" ${off ? 'disabled' : ''}>${ic(icon)}<span><b>${esc(label)}</b>${hint ? `<span class="note">${hint}</span>` : ''}</span></button>`;
    const local = g.local_hash ? `${esc(ago(g.local_mtime))} · ${human(g.bytes)}` : h('game.no_local');
    const cloud = g.cloud ? `${esc(ago(g.cloud.created_utc))} · ${esc(g.cloud.device_name)}` : h('games.cloud_none');
    modal(`<div class="ghead">${art('header', g.appid, g.name, 'hdr')}<div><h3>${esc(g.name)}</h3><div class="note">${statusPill(g.status)}</div></div></div>
      ${g.running ? `<div>${gameFlags(g)}</div>` : ''}
      <div class="gmenu">
        ${item('backup', 'upload', t('common.backup'), local, dis || !g.local_hash)}
        ${item('restore', 'download', t('common.restore'), cloud, dis || !g.cloud)}
        ${g.steam_ids && g.steam_ids.length ? item('migrate', 'swap', t('games.move_btn'), h('games.move_title'), g.running || S.busy) : ''}
        ${g.snapshots ? item('snaps', 'history', t('snap.row'), h('snap.count', { n: g.snapshots }), false) : ''}
        ${item('pickOne', 'check', t(sel().has(g.appid) ? 'gmenu.unselect' : 'gmenu.select'), h('gmenu.select_hint'), false)}
        ${item('gameMore', 'more', t('gmenu.details'), h('game.more_title'), false)}
      </div>
      <div class="mfoot"><button class="btn" data-close="1">${h('common.close')}</button></div>`);
  }

  /** What the bug report says about the game: English and plain, it goes to the public tracker. */
  function gameReportText(g) {
    const iso = (ms) => (ms ? new Date(ms).toISOString().replace('T', ' ').slice(0, 16) + ' UTC' : '-');
    return [
      `Name: ${g.name}`,
      `AppID: ${g.appid}`,
      `Status: ${g.status}${g.running ? ' (running)' : ''}`,
      `In manifest: ${g.has_rules ? 'yes' : 'no'}`,
      `Save folders: ${g.roots.length ? '\n  ' + g.roots.join('\n  ') : 'none found'}`,
      `Local: ${g.local_hash ? `${g.files} files, ${human(g.bytes)}, modified ${iso(g.local_mtime)}` : 'no save'}`,
      `Cloud: ${g.cloud ? `${g.versions} version${g.versions === 1 ? '' : 's'}, latest ${iso(g.cloud.created_utc)} from a ${g.cloud.os} PC` : 'none'}`,
      `Tied to a SteamID: ${g.steamid_bound ? 'yes' : 'no'}`,
      `Storage: ${S.info ? S.info.config.backend.kind : '?'}`,
    ].join('\n');
  }

  function confirmDeleteGame(g) {
    modal(`<h3>${h('game.delete_title', { game: g.name })}</h3>
      <div class="alert bad"><b>${h('game.delete_warn', { n: g.versions, game: g.name, where: storageName() })}</b><span>${h('game.delete_tail')}</span></div>
      <div class="mfoot"><button class="btn" data-close="1" data-autofocus>${h('common.cancel')}</button>
        <button class="btn danger" data-act="delCloudGo" data-id="${g.appid}">${ic('trash')}${h('game.delete_btn', { n: g.versions })}</button></div>`);
  }

  async function deleteGame(g) {
    busyModal(t('game.delete_title', { game: g.name }));
    S.busy = true;
    try {
      const r = await call('delete_cloud_game', { appid: g.appid });
      S.busy = false; close(); toast(r.message);
    } catch (e) { S.busy = false; fail(t('err.delete_failed'), e); }
    await loadGames();
  }

  function confirmDeleteAll() {
    modal(`<h3>${h('settings.delete_all')}</h3>
      <div class="alert bad"><b>${h('settings.delete_all_warn', { where: storageName() })}</b><span>${h('settings.delete_all_tail')}</span></div>
      <label class="check" for="delSure"><input type="checkbox" id="delSure"> <span>${h('settings.delete_all_confirm')}</span></label>
      <div class="mfoot"><button class="btn" data-close="1" data-autofocus>${h('common.cancel')}</button>
        <button class="btn danger" data-act="delAllGo" disabled>${ic('trash')}${h('settings.delete_all_btn')}</button></div>`);
    $('#delSure').addEventListener('change', (e) => { layer.querySelector('[data-act="delAllGo"]').disabled = !e.target.checked; });
  }

  async function deleteAll() {
    busyModal(t('settings.delete_all'));
    S.busy = true;
    try {
      const r = await call('delete_cloud_all');
      S.busy = false; close(); toast(r.message);
    } catch (e) { S.busy = false; fail(t('err.delete_failed'), e); }
    S.list = null; loadGames();
    if (S.view === 'settings') render();
  }

  // ---------- earlier saves on this PC (local snapshots) ----------
  // The backend keeps the save that a restore overwrites (sync::KEEP_SNAPSHOTS per game).
  const KEEP_SNAPSHOTS = 10;
  let SN = null; // { appid, list, sel }

  /** sel: a snapshot id, 'newest' (the save just replaced, for Undo), or nothing (newest that differs). */
  async function openSnapshots(appid, sel) {
    const g = game(appid);
    if (!g) return;
    busyModal(t('snap.title', { game: g.name }));
    let list;
    try { list = await call('list_snapshots', { appid }); } catch (e) { return fail(t('err.snapshots_failed'), e); }
    const pickDefault = list.find((x) => x.id === sel) || (sel === 'newest' && list[0]) || list.find((x) => !sameAsLocal(g, x)) || list[0];
    SN = { appid, list, sel: pickDefault ? pickDefault.id : null };
    renderSnapshots();
  }

  const sameAsLocal = (g, s) => !!(g.local_hash && s.content_hash && s.content_hash === g.local_hash);
  const snapWhen = (s) => s.save_mtime || s.created_utc;

  function snapReason(s) {
    if (s.reason === 'restore') return h('snap.reason_restore', { device: s.source_device || '?', time: fmt(s.source_time) });
    if (s.reason === 'snapshot') return h('snap.reason_snapshot', { time: fmt(s.source_time) });
    return h('snap.reason_unknown');
  }

  function renderSnapshots() {
    const g = SN && game(SN.appid);
    if (!g) return close();
    const rows = SN.list.map((s) => {
      const id = 'sn-' + s.id;
      return `<label class="srow ${s.id === SN.sel ? 'on' : ''}" for="${esc(id)}">
        <input type="radio" name="snap" id="${esc(id)}" data-snap="${esc(s.id)}" ${s.id === SN.sel ? 'checked' : ''}>
        <span class="sinfo"><span class="stime">${h(s.save_mtime ? 'snap.played' : 'snap.kept', { time: fmt(snapWhen(s)) })}<span class="note"> · ${esc(ago(snapWhen(s)))}</span></span>
          <span class="note">${snapReason(s)}${s.save_mtime ? ' · ' + h('snap.kept_at', { time: fmt(s.created_utc) }) : ''}</span>
          <span class="note">${esc(files(s.files))} · ${human(s.bytes)}</span></span>
        ${sameAsLocal(g, s) ? statusPill('sync', t('snap.same')) : ''}</label>`;
    }).join('');
    const has = SN.list.length > 0;
    modal(`<div class="ghead">${art('header', g.appid, g.name, 'hdr')}<div><h3>${h('snap.title', { game: g.name })}</h3><div class="note">${h('snap.intro', { n: KEEP_SNAPSHOTS })}</div></div></div>
      ${g.running ? `<div class="alert bad">${h('snap.running', { game: g.name })}</div>` : ''}
      ${has ? `<div class="glist snaps" role="radiogroup" aria-label="${h('snap.row')}">${rows}</div><div id="snapDetail"></div>` : `<div class="empty">${h('snap.empty')}</div>`}
      <div class="mfoot"><button class="btn" data-act="snapFolder" data-id="${g.appid}">${ic('folder')}${h('snap.open_folder')}</button>
        ${has ? `<button class="btn" data-act="snapDel">${ic('trash')}${h('snap.delete_btn')}</button>` : ''}<span class="spacer"></span>
        <button class="btn" data-close="1" ${has ? '' : 'data-autofocus'}>${h('common.close')}</button>
        ${has ? `<button class="btn primary" data-act="snapGo">${ic('history')}${h('snap.restore_btn')}</button>` : ''}</div>`);
    layer.querySelectorAll('[data-snap]').forEach((r) => r.addEventListener('change', () => { SN.sel = r.dataset.snap; snapDetail(); }));
    snapDetail();
    // The focused button may have just been disabled (e.g. the chosen save is the current one)
    const f = document.activeElement;
    if (!f || f === document.body || f.disabled) {
      const r = layer.querySelector('[data-snap]:checked') || layer.querySelector('[data-close]');
      if (r) r.focus();
    }
  }

  // The part that depends on the chosen snapshot, updated in place so the list keeps its scroll and focus
  function snapDetail() {
    const g = game(SN.appid), s = SN.list.find((x) => x.id === SN.sel);
    layer.querySelectorAll('.srow').forEach((el) => el.classList.toggle('on', !!s && el.htmlFor === 'sn-' + s.id));
    const out = $('#snapDetail');
    const go = layer.querySelector('[data-act="snapGo"]'), del = layer.querySelector('[data-act="snapDel"]');
    if (go) go.disabled = !s || sameAsLocal(g, s) || g.running || S.busy;
    if (del) del.disabled = !s || S.busy;
    if (!out || !s) return;
    const notes = [];
    if (sameAsLocal(g, s)) notes.push(h('snap.note_same'));
    else {
      notes.push(g.local_hash ? h('snap.note_keep') : h('snap.note_no_local'));
      if (s.reason) notes.push(h('snap.note_exact'));
      if (g.cloud) notes.push(h('snap.note_cloud', { status: t('status.local') }));
    }
    out.innerHTML = `<div class="cmp">${box(t('common.local_this_pc'), g.local_mtime, g.local_hash ? `${ago(g.local_mtime)} · ${files(g.files)} · ${human(g.bytes)}` : t('restore.no_save'), false)}${box(t('snap.chosen'), snapWhen(s), `${ago(snapWhen(s))} · ${files(s.files)} · ${human(s.bytes)}`, false)}</div>
      <p class="note">${notes.join(' ')}</p>`;
  }

  async function restoreSnapshot() {
    const g = game(SN.appid), s = SN.list.find((x) => x.id === SN.sel);
    if (!g || !s) return;
    busyModal(t('snap.going_back', { game: g.name }));
    S.busy = true;
    try {
      const r = await call('restore_snapshot', { appid: g.appid, id: s.id });
      S.busy = false; close();
      // The save just replaced is now the newest snapshot: Undo opens the list on it
      toast(r.message, false, r.outcome === 'restored' && g.local_hash ? { label: t('snap.undo'), run: () => openSnapshots(g.appid, 'newest') } : null);
    } catch (e) { S.busy = false; fail(t('err.snapshot_restore_failed'), e); }
    await loadGames();
  }

  function confirmDeleteSnapshot() {
    const s = SN.list.find((x) => x.id === SN.sel);
    if (!s) return;
    modal(`<h3>${h('snap.delete_title')}</h3>
      <div class="alert bad"><span>${h('snap.delete_warn', { time: fmt(snapWhen(s)) })}</span></div>
      <div class="mfoot"><button class="btn" data-act="snapBack" data-autofocus>${h('common.cancel')}</button>
        <button class="btn danger" data-act="snapDelGo">${ic('trash')}${h('snap.delete_btn')}</button></div>`);
  }

  async function deleteSnapshot() {
    const id = SN.sel;
    try { await call('delete_snapshot', { appid: SN.appid, id }); }
    catch (e) { return fail(t('err.snapshot_delete_failed'), e); }
    toast(t('snap.deleted'));
    const i = SN.list.findIndex((x) => x.id === id);
    SN.list = SN.list.filter((x) => x.id !== id);
    const next = SN.list[Math.min(i, SN.list.length - 1)];
    SN.sel = next ? next.id : null;
    renderSnapshots();
    loadGames(); // the count in the game's details
  }

  async function addToSteam() {
    busyModal(t('settings.add_to_steam'));
    try {
      const r = await call('add_to_steam');
      close(); toast(t(r.added ? 'settings.added_to_steam' : 'settings.updated_in_steam', { account: r.account }));
    } catch (e) { fail(t('err.add_to_steam_failed'), e); }
  }

  // ---------- events ----------
  document.addEventListener('click', async (e) => {
    if (e.target.dataset && e.target.dataset.scrim) { close(); return; }
    const cover = e.target.closest('[data-cover]');
    if (cover && sel().size && !e.target.closest('.pick')) { const id = +cover.dataset.cover; setPicked([id], !sel().has(id)); return; }
    const n = e.target.closest('[data-view],[data-f],[data-act],[data-close]');
    if (!n || n.disabled) return;
    if (n.dataset.close) return close();
    if (n.dataset.view) {
      S.view = n.dataset.view; render();
      if (S.view === 'games' && !S.list && !S.loading) loadGames();
      return;
    }
    if (n.dataset.f) { S.filter = n.dataset.f; return render(); }
    const a = n.dataset.act, id = +n.dataset.id;
    try {
      if (a === 'reload') { await loadInfo(); await loadGames(); }
      else if (a === 'backup') startBackup(game(id));
      else if (a === 'restore') openRestore(game(id));
      else if (a === 'forceBackup') doOp('backup', game(id), { force: true });
      else if (a === 'toRestore') openRestore(game(id));
      else if (a === 'toBackup') { close(); confirmOrBackup(game(id)); }
      else if (a === 'doRestore') {
        const sel = $('#remapTo');
        const remap = sel && sel.value !== sel.dataset.from ? { remapFrom: sel.dataset.from, remapTo: sel.value } : {};
        doOp('restore', game(id), { versionId: n.dataset.ver, force: true, ...remap });
      }
      else if (a === 'gameMore') openGame(game(id));
      else if (a === 'delCloud') confirmDeleteGame(game(id));
      else if (a === 'snaps') openSnapshots(id);
      else if (a === 'pickOne') { close(); setPicked([id], !sel().has(id)); S.lastKey = `[data-card="${id}"]`; restoreFocus(); }
      else if (a === 'snapGo') restoreSnapshot();
      else if (a === 'snapDel') confirmDeleteSnapshot();
      else if (a === 'snapDelGo') deleteSnapshot();
      else if (a === 'snapBack') renderSnapshots();
      else if (a === 'snapFolder') await call('open_snapshot_folder', { appid: id });
      else if (a === 'delCloudGo') deleteGame(game(id));
      else if (a === 'delAll') confirmDeleteAll();
      else if (a === 'delAllGo') deleteAll();
      else if (a === 'addSteam') addToSteam();
      else if (a === 'copyLaunch') {
        const inp = $('#launchOpt');
        try { await navigator.clipboard.writeText(inp.value); toast(t('game.copied')); }
        catch (err) { inp.select(); toast(t('rclone.link_copy_failed'), true); }
      }
      else if (a === 'reportGame') {
        const g = game(id);
        reportIssue('bug', `[Bug] ${g.name}: `, null, { appid: g.appid, gameName: g.name, details: gameReportText(g) });
      }
      else if (a === 'reveal') {
        const inp = $('#' + n.dataset.for); if (!inp) return;
        const show = inp.type === 'password';
        inp.type = show ? 'text' : 'password';
        n.setAttribute('aria-pressed', String(show));
        n.title = t(show ? 'rclone.hide' : 'rclone.show'); n.setAttribute('aria-label', n.title);
        n.innerHTML = ic(show ? 'eyeOff' : 'eye');
      }
      else if (a === 'migrate') openMigrate({ appid: id });
      else if (a === 'migrateAcct') openMigrate({ from: n.dataset.sid });
      else if (a === 'wzBack') { W.step = 1; renderWizard(); }
      else if (a === 'wzNext') wizardPrepare();
      else if (a === 'wzApply') wizardApply();
      else if (a === 'migKeep' || a === 'migUndo') migDecide(a === 'migKeep' ? 'keep' : 'undo', n.dataset.mid);
      else if (a === 'backupAll') openAll('backup');
      else if (a === 'restoreAll') openAll('restore');
      else if (a === 'runAll') runAll();
      else if (a === 'planAll' || a === 'planNone') {
        P.plan.filter((p) => p.run).forEach((p) => (a === 'planAll' ? P.on.add(p.g.appid) : P.on.delete(p.g.appid)));
        layer.querySelectorAll('[data-plan]').forEach((c) => { c.checked = a === 'planAll'; });
        refreshPlanFoot();
      }
      else if (a === 'clearSel') { sel().clear(); renderGames(); }
      else if (a === 'manifest') {
        busyModal(t('settings.manifest_update'));
        try { const c = await call('update_manifest'); close(); toast(t('settings.manifest_done', { n: c.toLocaleString(locale()) })); await loadInfo(); await loadGames(); }
        catch (err) { fail(t('err.manifest_failed'), err); }
      }
      else if (a === 'pick') { const p = await call('pick_folder'); if (p) $('#fPath').value = p; }
      else if (a === 'rinstall') {
        busyModal(t('rclone.install'));
        $('#busyText').textContent = t('rclone.installing');
        S.busy = true;
        try { const v = await call('rclone_install'); close(); toast(t('rclone.installed', { version: v })); loadRclone(); }
        catch (err) { fail(t('err.rclone_install_failed'), err); }
        S.busy = false;
      }
      else if (a === 'rconnect') {
        const clientId = $('#cid').value.trim(), clientSecret = $('#csec').value.trim();
        if (!clientId || !clientSecret) { toast(t('rclone.need_both'), true); return; }
        busyModal(t('rclone.connect'));
        $('#busyText').textContent = t('rclone.signing_in');
        layer.querySelector('.modal').insertAdjacentHTML('beforeend', '<div id="authLink" hidden></div>');
        S.busy = true;
        const poll = setInterval(showAuthLink, 700);
        try {
          const r = await call('rclone_connect', { clientId, clientSecret });
          clearInterval(poll);
          S.busy = false; close(); toast(t('rclone.connected_toast', { remote: r }));
          await loadInfo(); S.list = null; render(); loadGames();
        } catch (err) { clearInterval(poll); S.busy = false; fail(t('err.connect_failed'), err); loadRclone(); }
      }
      else if (a === 'layout') { S.layout = n.dataset.layout === 'list' ? 'list' : 'grid'; store.set('cp.layout', S.layout); renderGames(); }
      else if (a === 'updLater') { S.updateHidden = true; render(); }
      else if (a === 'updShow') { if (S.update) updateModal(S.update); }
      else if (a === 'checkUpd') { await checkUpdate(true); }
      else if (a === 'updInstall') { await installUpdate(); }
      else if (a === 'updPage') { try { await call('open_release_page'); } catch (e) { toast(String(e), true); } }
      else if (a === 'copyAuth') {
        const url = $('#authUrl') && $('#authUrl').value;
        try { await navigator.clipboard.writeText(url); toast(t('rclone.link_copied')); }
        catch (e) { $('#authUrl').select(); toast(t('rclone.link_copy_failed'), true); }
      }
      else if (a === 'openAuth') {
        try { await call('open_auth_url'); } catch (e) { toast(String(e), true); }
      }
      else if (a === 'saveCfg') {
        const cfg = readSettings();
        await call('save_config', { config: cfg });
        await loadStrings();
        await loadInfo(); toast(t('settings.saved')); S.list = null; S.accounts = null; render(); loadGames(); loadAccounts();
      }
      else if (a === 'report') openReport();
      // Gaming Mode has no window close button: quit from the sidebar (not in the middle of a backup)
      else if (a === 'pCloud' || a === 'pLocal' || a === 'pCancel') {
        pane.querySelectorAll('button').forEach((b) => { b.disabled = true; });
        await call('prompt_answer', { choice: a === 'pCloud' ? 'cloud' : a === 'pLocal' ? 'local' : 'cancel' });
      }
      else if (a === 'unkeep') {
        n.disabled = true;
        try { const r = await call('unkeep_version', { appid: Number(n.dataset.id), id: n.dataset.ver }); toast(r.message); }
        catch (e) { fail(t('err.unkeep_failed'), e); }
        if (S.view === 'history') renderHistory();
      }
      else if (a === 'quit') { if (S.busy) toast(t('nav.quit_busy'), true); else await call('quit_app'); }
      else if (a === 'reportBug') reportIssue('bug', '[Bug] ', null);
      else if (a === 'reportFeature') reportIssue('feature', '[Feature] ', null);
      else if (a === 'issueFromError') reportIssue('bug', `[Bug] ${S.lastError ? S.lastError.title : ''}`, S.lastError ? S.lastError.msg : null);
      else if (a === 'openLogs') await call('open_log_folder');
      else if (a === 'copyLog') copyLog();
    } catch (err) { fail(t('err.generic'), err); }
  });

  document.addEventListener('change', (e) => {
    const c = e.target;
    if (c.dataset && c.dataset.pick) setPicked([+c.dataset.pick], c.checked);
    else if (c.dataset && c.dataset.pickall) setPicked((S.shown || []).map((g) => g.appid), c.checked);
  });

  // ---------- error reporting ----------
  function logUi(level, message) {
    if (!invoke) return;
    invoke('log_frontend', { level, message: String(message) }).catch(() => {});
  }
  window.addEventListener('error', (e) => logUi('error', `JS: ${e.message} @ ${e.filename}:${e.lineno}:${e.colno}`));
  window.addEventListener('unhandledrejection', (e) => logUi('error', `Promise: ${e.reason && e.reason.stack ? e.reason.stack : e.reason}`));

  /** Error dialog with a Create issue button. The backend already logged the command's error; this adds UI context. */
  function fail(title, e) {
    const msg = String(e && e.message ? e.message : e);
    // The message itself is in the UI language; the backend already logged the English version.
    logUi('warn', `Error dialog shown (view=${S.view})`);
    S.lastError = { title, msg };
    S.busy = false;
    modal(`<h3>${esc(title)}</h3>
      <div class="alert bad"><span class="errmsg">${esc(msg)}</span></div>
      <p class="note">${h('report.error_hint')}</p>
      <div class="mfoot"><button class="btn" data-act="openLogs">${ic('folder')}${h('report.open_logs')}</button><button class="btn" data-act="copyLog">${ic('copy')}${h('report.copy_log')}</button><span class="spacer"></span>
        <button class="btn" data-close="1">${h('common.close')}</button><button class="btn primary" data-act="issueFromError">${h('report.create_issue')}</button></div>`);
  }

  function openReport() {
    const v = S.info ? S.info.version : '';
    modal(`<h3>${h('nav.report')}</h3>
      <p>${h('report.intro')}</p>
      <div class="cards">
        <div class="card"><b>${h('report.bug')}</b><span class="note">${h('report.bug_hint')}</span><div><button class="btn primary" data-act="reportBug">${ic('bug')}${h('report.bug')}</button></div></div>
        <div class="card"><b>${h('report.feature')}</b><span class="note">${h('report.feature_hint')}</span><div><button class="btn" data-act="reportFeature">${ic('sparkle')}${h('report.feature_btn')}</button></div></div>
      </div>
      <p class="note">${h('report.log_location', { path: S.info ? S.info.log_dir : '', version: v })}</p>
      <div class="mfoot"><button class="btn" data-act="openLogs">${ic('folder')}${h('report.open_logs')}</button><button class="btn" data-act="copyLog">${ic('copy')}${h('report.copy_log')}</button><span class="spacer"></span><button class="btn" data-close="1">${h('common.close')}</button></div>`);
  }

  async function reportIssue(kind, title, error, gameInfo) {
    try {
      const r = await call('report_issue', { kind, title, error, appid: null, gameName: null, details: null, ...(gameInfo || {}) });
      toast(t('report.opened'));
      if (r.log_truncated) toast(t('report.truncated'));
    } catch (e) { toast(String(e), true); }
  }

  // ---------- updates ----------
  // manual=false: the silent check at startup (the backend skips it when turned off).
  async function checkUpdate(manual) {
    if (manual) busyModal(t('settings.check_now'));
    let u = null;
    try { u = await call('check_update', { manual }); }
    catch (e) { if (manual) fail(t('err.update_failed'), e); return; }
    S.update = u;
    if (u) S.updateHidden = false;
    if (manual) {
      if (u) updateModal(u);
      else { close(); toast(t('update.none', { version: S.info ? S.info.version : '' })); }
    }
    if (S.view === 'games') render();
  }
  // Release notes are Markdown: escape everything, then style headings, **bold** and `code` only.
  function mdLite(text) {
    return text.split('\n').map((line) => {
      let l = esc(line);
      const head = /^#{1,6}\s+(.*)$/.exec(l);
      if (head) return `<b>${head[1]}</b>`;
      l = l.replace(/^(\s*)[-*]\s+/, '$1• ');
      return l.replace(/\*\*(.+?)\*\*/g, '<b>$1</b>').replace(/`([^`]+)`/g, '<code>$1</code>');
    }).join('\n');
  }
  function updateModal(u) {
    const notes = u.notes ? `<h4>${h('update.whats_new')}</h4><div class="notes">${mdLite(u.notes)}</div>` : '';
    const how = u.can_install
      ? `<p class="note">${h('update.confirm')}${S.info && S.info.os === 'windows' ? ' ' + h('update.confirm_win') : ''}</p>`
      : `<div class="alert warn">${h('update.manual_hint')}</div>`;
    const go = u.can_install
      ? `<button class="btn primary" data-act="updInstall" ${S.busy ? 'disabled' : ''}>${ic('sparkle')}${h('update.install')}</button>`
      : `<button class="btn primary" data-act="updPage">${ic('external')}${h('update.download')}</button>`;
    modal(`<h3>${h('update.title', { version: u.version })}</h3>
      <p>${h('update.available', { version: u.version, current: u.current })}</p>${notes}${how}
      <div class="mfoot"><button class="btn" data-close="1">${h('update.later')}</button>${go}</div>`);
  }
  async function installUpdate() {
    const u = S.update;
    if (!u || S.busy) return;
    busyModal(t('update.title', { version: u.version }));
    $('#busyText').textContent = t('update.downloading');
    S.busy = true;
    const poll = setInterval(async () => {
      let p; try { p = await call('update_progress'); } catch (e) { return; }
      const el = $('#busyText'); if (!el) return;
      if (p.total && p.downloaded >= p.total) el.textContent = t('update.installing');
      else if (p.total) el.textContent = t('update.downloading_pct', { pct: Math.floor(p.downloaded * 100 / p.total) });
    }, 500);
    try {
      await call('install_update', { version: u.version });
      // Linux restarts and Windows exits before this returns; if it does return, the installer is running.
      clearInterval(poll);
      const el = $('#busyText'); if (el) el.textContent = t('update.installing');
    } catch (e) {
      clearInterval(poll);
      S.busy = false;
      fail(t('err.update_failed'), e);
    }
  }

  // Google sign-in link printed by rclone; shown so it can be copied when no browser opened.
  async function showAuthLink() {
    const box = $('#authLink');
    if (!box || !box.hidden) return;
    let url = null;
    try { url = await call('rclone_auth_url'); } catch (e) { return; }
    if (!url || !$('#authLink') || !box.hidden) return;
    box.innerHTML = `<p class="note">${h('rclone.link_hint')}</p>
      <input id="authUrl" class="mono" readonly value="${esc(url)}" style="width:100%">
      <div class="row"><button class="btn primary" data-act="copyAuth">${ic('copy')}${h('rclone.copy_link')}</button>
      <button class="btn" data-act="openAuth">${ic('external')}${h('rclone.open_link')}</button></div>`;
    box.hidden = false;
  }

  async function copyLog() {
    const text = await call('get_log', { lines: 2000 });
    try { await navigator.clipboard.writeText(text); toast(t('report.copied')); }
    catch (e) { toast(t('report.copy_failed'), true); }
  }
  function confirmOrBackup(g) {
    if (g.status === 'local' || g.status === 'new') doOp('backup', g, {});
    else confirmBackup(g);
  }
  // ---------- controller and arrow-key navigation ----------
  // For Gaming Mode on Bazzite / SteamOS and handhelds (and any gamepad). Each game is one stop:
  // in the grid the D-pad moves card by card in reading order (right from the last card of a row
  // goes to the first of the next row), up/down by whole rows; the small buttons on a card are
  // skipped, and A opens the game's menu instead. Elsewhere the focus moves to the nearest control
  // in that direction. A presses, B closes the dialog or goes back to the sidebar, X selects the
  // focused game, LB / RB switch screens. Arrow keys, Enter, Space and Esc do the same.
  const FOCUSABLE = 'button:not([disabled]),input:not([disabled]),select:not([disabled]),summary,a[href],[data-card]';
  const VIEWS = ['games', 'accounts', 'history', 'settings'];
  const usingPad = () => document.body.classList.contains('pad');

  function focusables() {
    const scope = layer.querySelector('.modal') || document;
    return [...scope.querySelectorAll(FOCUSABLE)].filter((el) => {
      if (el.closest('[hidden]') || (el.closest('details:not([open])') && el.tagName !== 'SUMMARY')) return false;
      // Controls inside a game card or row are reached through the game's menu (A), not one by one
      const card = el.closest('[data-card]');
      if (card && card !== el) return false;
      if (el.closest('#toasts')) return false;
      const r = el.getBoundingClientRect();
      return r.width > 0 && r.height > 0;
    });
  }

  function focusEl(el) {
    if (!el) return;
    if (!layer.contains(el)) S.lastKey = keyOf(el);
    el.focus({ preventScroll: true });
    el.scrollIntoView({ block: 'nearest', inline: 'nearest' });
  }

  /** Nearest control whose centre lies in direction `dir`, preferring ones in line with `from`. */
  function nearest(from, dir, items) {
    const a = from.getBoundingClientRect();
    const ax = a.left + a.width / 2, ay = a.top + a.height / 2;
    let best = null, bestScore = Infinity;
    for (const el of items) {
      if (el === from) continue;
      const b = el.getBoundingClientRect();
      const dx = b.left + b.width / 2 - ax, dy = b.top + b.height / 2 - ay;
      const main = dir === 'left' ? -dx : dir === 'right' ? dx : dir === 'up' ? -dy : dy;
      const cross = dir === 'left' || dir === 'right' ? Math.abs(dy) : Math.abs(dx);
      if (main <= 4) continue;
      const score = main + cross * 2.5;
      if (score < bestScore) { bestScore = score; best = el; }
    }
    return best;
  }

  /** Cards in a grid or rows in the list: move by position in the list, not by pixels. */
  function moveInGrid(cur, dir, items) {
    const grid = cur.closest('[data-navgrid]');
    const cards = [...grid.querySelectorAll('[data-card]')].filter((c) => items.includes(c));
    const i = cards.indexOf(cur);
    // Cards per row: how many share the first card's top edge (the list view has one per row).
    // offsetTop ignores the lift transform of the focused card.
    const top = (c) => c.offsetTop;
    let cols = cards.findIndex((c) => top(c) !== top(cards[0]));
    if (cols < 1) cols = cards.length;
    const row = Math.floor(i / cols), lastRow = Math.floor((cards.length - 1) / cols);
    let j = null;
    // One per row (the list view): left goes to the sidebar, right does nothing
    if (dir === 'right') j = cols > 1 && i + 1 < cards.length ? i + 1 : null;
    else if (dir === 'left') j = cols > 1 && i > 0 ? i - 1 : 'nav';
    else if (dir === 'down') j = i + cols < cards.length ? i + cols : row < lastRow ? cards.length - 1 : 'out';
    else if (dir === 'up') j = row > 0 ? i - cols : 'out';
    if (typeof j === 'number') return focusEl(cards[j]);
    if (j === 'nav') return focusEl(document.querySelector(`#nav button[data-view="${S.view}"]`));
    // Leaving the grid (up to the filters, down past the last row): the closest row of controls
    // outside it, then the control in that row nearest to the game
    if (j !== 'out') return;
    const a = cur.getBoundingClientRect(), cx = a.left + a.width / 2;
    // The sidebar is left of the grid (Left from the first game, or B), never above or below it
    const outside = items.filter((el) => !grid.contains(el) && !el.closest('#nav')).map((el) => ({ el, r: el.getBoundingClientRect() }))
      .filter(({ r }) => (dir === 'up' ? r.bottom <= a.top + 4 : r.top >= a.bottom - 4));
    if (!outside.length) return;
    const gap = ({ r }) => (dir === 'up' ? a.top - r.bottom : r.top - a.bottom);
    const minGap = Math.min(...outside.map(gap));
    const line = outside.filter((o) => gap(o) <= minGap + 12);
    line.sort((p, q) => Math.abs(p.r.left + p.r.width / 2 - cx) - Math.abs(q.r.left + q.r.width / 2 - cx));
    focusEl(line[0].el);
  }

  function moveFocus(dir) {
    const items = focusables();
    if (!items.length) return;
    const cur = document.activeElement;
    if (!cur || !items.includes(cur)) {
      // Nothing focused yet: start in the dialog, on the first game, or on the screen's sidebar button
      const firstCard = !layer.querySelector('.modal') && items.find((el) => el.dataset.card);
      return focusEl(layer.querySelector('.modal') ? items[0] : firstCard || document.querySelector(`#nav button[data-view="${S.view}"]`) || items[0]);
    }
    if (cur.dataset.card && cur.closest('[data-navgrid]')) return moveInGrid(cur, dir, items);
    const next = nearest(cur, dir, items);
    if (next) focusEl(next);
  }

  // A stable description of a control, to focus it again after the screen re-renders
  function keyOf(el) {
    if (!el || el === document.body) return null;
    if (el.id) return '#' + CSS.escape(el.id);
    const attrs = ['data-card', 'data-act', 'data-id', 'data-view', 'data-f', 'data-mid', 'data-layout', 'data-for', 'data-pick', 'data-pickall', 'data-plan', 'data-snap'].filter((k) => el.hasAttribute(k));
    return attrs.length ? attrs.map((k) => `[${k}="${CSS.escape(el.getAttribute(k))}"]`).join('') : null;
  }
  function refocus(key) {
    setTimeout(() => {
      if (document.activeElement && document.activeElement !== document.body) return;
      const items = focusables();
      focusEl((key && items.find((el) => el.matches(key))) || items[0]);
    }, 80);
  }
  // Where the focus was outside dialogs, to return there when a dialog closes or the screen re-renders
  document.addEventListener('focusin', (e) => { if (!layer.contains(e.target)) S.lastKey = keyOf(e.target); });
  function restoreFocus() {
    if (usingPad() && !layer.querySelector('.modal') && S.lastKey) refocus(S.lastKey);
  }

  function press() {
    const el = document.activeElement;
    if (!el || el === document.body) return moveFocus('down');
    if (el.dataset.card) return openGameMenu(+el.dataset.card);
    if (el.tagName === 'SELECT') {
      // A native drop-down cannot be opened from script: A picks the next option instead
      el.selectedIndex = (el.selectedIndex + 1) % el.options.length;
      el.dispatchEvent(new Event('change', { bubbles: true }));
      return;
    }
    if (el.tagName === 'INPUT' && !['checkbox', 'radio'].includes(el.type)) return; // typing: Steam keyboard (Steam + X)
    const key = keyOf(el);
    el.click();
    refocus(key);
  }

  /** X: select or unselect the focused game (for Back up / Restore N games). */
  function pickFocused() {
    const el = document.activeElement;
    if (!el || !el.dataset || !el.dataset.card || layer.querySelector('.modal')) return;
    const id = +el.dataset.card;
    setPicked([id], !sel().has(id));
    refocus(`[data-card="${id}"]`);
  }

  function back() {
    if (S.busy) return;
    if (layer.querySelector('.modal')) { close(); return; }
    focusEl(document.querySelector(`#nav button[data-view="${S.view}"]`));
  }

  function switchView(step) {
    if (layer.querySelector('.modal') || S.busy) return;
    const i = VIEWS.indexOf(S.view);
    const next = document.querySelector(`#nav button[data-view="${VIEWS[(i + step + VIEWS.length) % VIEWS.length]}"]`);
    if (next) { next.click(); focusEl(next); }
  }

  // Steam Input can send the D-pad both as a gamepad and as arrow keys: one press must move once
  const lastMove = { dir: null, at: 0, src: null };
  function navMove(dir, src) {
    const now = performance.now();
    if (lastMove.dir === dir && lastMove.src !== src && now - lastMove.at < 180) return;
    Object.assign(lastMove, { dir, at: now, src });
    moveFocus(dir);
  }

  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') { if (layer.querySelector('.modal')) { if (!S.busy) close(); } else back(); return; }
    const el = document.activeElement;
    if (el && el.dataset && el.dataset.card && !e.altKey && !e.ctrlKey && !e.metaKey) {
      if (e.key === 'Enter') { e.preventDefault(); return press(); }
      if (e.key === ' ') { e.preventDefault(); return pickFocused(); }
    }
    const dir = { ArrowUp: 'up', ArrowDown: 'down', ArrowLeft: 'left', ArrowRight: 'right' }[e.key];
    if (!dir || e.altKey || e.ctrlKey || e.metaKey) return;
    // Leave arrows to drop-downs, number fields and the text cursor
    if (el && (el.tagName === 'SELECT' || el.type === 'number' || (el.tagName === 'INPUT' && ['text', 'password', 'search'].includes(el.type) && (dir === 'left' || dir === 'right')))) return;
    e.preventDefault();
    document.body.classList.add('pad');
    navMove(dir, 'key');
  });

  // Gamepad API (standard mapping): 0 A, 1 B, 2 X, 4 LB, 5 RB, 12–15 D-pad, axes 0/1 left stick
  const pad = { held: {}, next: {}, raf: 0, at: 0 };
  function padLoop(now) {
    const gp = [...(navigator.getGamepads ? navigator.getGamepads() : [])].find((g) => g && g.connected);
    if (!gp) { pad.raf = 0; return; }
    const btn = (i) => !!(gp.buttons[i] && gp.buttons[i].pressed);
    const ax = (i) => gp.axes[i] || 0;
    const inputs = {
      up: btn(12) || ax(1) < -0.6, down: btn(13) || ax(1) > 0.6, left: btn(14) || ax(0) < -0.6, right: btn(15) || ax(0) > 0.6,
      a: btn(0), b: btn(1), x: btn(2), lb: btn(4), rb: btn(5),
    };
    for (const [k, on] of Object.entries(inputs)) {
      if (!on) { pad.held[k] = false; continue; }
      const first = !pad.held[k];
      // Directions repeat while held (first after 400 ms, then every 130 ms); buttons fire once
      if (first || (['up', 'down', 'left', 'right'].includes(k) && now >= pad.next[k])) {
        pad.next[k] = now + (first ? 400 : 130);
        pad.held[k] = true;
        pad.at = now;
        document.body.classList.add('pad');
        if (k === 'a') press();
        else if (k === 'b') back();
        else if (k === 'x') pickFocused();
        else if (k === 'lb') switchView(-1);
        else if (k === 'rb') switchView(1);
        else navMove(k, 'pad');
      }
    }
    pad.raf = requestAnimationFrame(padLoop);
  }
  function startPad() { if (!pad.raf) pad.raf = requestAnimationFrame(padLoop); }
  window.addEventListener('gamepadconnected', startPad);
  // Some WebViews only report a pad that was already connected after a button press
  if (navigator.getGamepads && [...navigator.getGamepads()].some((g) => g)) startPad();
  // Focus rings and button hints while a controller or the arrow keys are used; a real mouse
  // movement turns them off (small or synthetic moves right after pad input are ignored)
  document.addEventListener('mousemove', (e) => {
    if (!usingPad() || performance.now() - pad.at < 800 || Math.abs(e.movementX) + Math.abs(e.movementY) < 4) return;
    document.body.classList.remove('pad');
  });

  // ---------- launch prompt ----------
  // `checkpoint prompt <file>` (prompt.rs): the Steam launch option found a conflicting save as the
  // game starts. This small window shows both sides and asks; the answer goes back to the launch
  // wrapper, which then restores, keeps this PC's save, or doesn't start the game.
  async function launchPrompt() {
    document.querySelector('.app').classList.add('prompt');
    const p = await call('prompt_info');
    const local = p.local_mtime || 0, cloud = p.cloud ? p.cloud.created_utc : 0;
    const item = (act, icon, label, hint, primary) => `<button class="btn gm${primary ? ' primary' : ''}" data-act="${act}">${ic(icon)}<span><b>${esc(label)}</b><span class="note">${esc(hint)}</span></span></button>`;
    const device = p.cloud ? p.cloud.device_name : '';
    pane.innerHTML = `<div class="prompt-box">
      <h2>${h('prompt.title', { game: p.name })}</h2>
      <div class="alert warn">${h(p.status === 'diverged' ? 'prompt.diverged' : 'prompt.conflict', { device })}</div>
      <div class="cmp">
        ${box(t('prompt.local'), local, `${files(p.local_files)} · ${human(p.local_bytes)}`, local > cloud)}
        ${box(t('prompt.cloud'), cloud, `${device} · ${files(p.cloud ? p.cloud.files : 0)} · ${human(p.cloud ? p.cloud.bytes : 0)}`, cloud > local)}
      </div>
      <div class="gmenu">
        ${item('pCloud', 'download', t('prompt.use_cloud'), t('prompt.use_cloud_hint', { device }), true)}
        ${item('pLocal', 'monitor', t('prompt.keep_local'), t('prompt.keep_local_hint'))}
        ${item('pCancel', 'minus', t('prompt.cancel'), t('prompt.cancel_hint'))}
      </div>
    </div>`;
    const first = pane.querySelector('[data-act="pCloud"]'); if (first) first.focus();
  }

  // ---------- boot ----------
  (async function boot() {
    document.querySelectorAll('[data-ic]').forEach((el) => { el.innerHTML = ic(el.dataset.ic); });
    if (window.__CHECKPOINT_PROMPT__) {
      try { await loadStrings(); await launchPrompt(); } catch (e) { pane.innerHTML = `<div class="banner bad"><p>${esc(e)}</p></div>`; }
      return;
    }
    try {
      await loadStrings();
      await loadInfo();
      loadAccounts();
      checkUpdate(false);
      if (!backendReady()) { S.view = 'settings'; render(); toast(t('settings.pick_storage_first')); loadGames(); return; }
      await loadGames();
    } catch (e) {
      pane.innerHTML = `<div class="banner bad"><p>${esc(e)}</p></div>`;
    }
  })();
})();

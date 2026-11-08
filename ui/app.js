(function () {
  'use strict';
  const invoke = window.__TAURI__ ? window.__TAURI__.core.invoke : null;
  const $ = (s) => document.querySelector(s);
  const pane = $('#pane'), layer = $('#layer');
  const esc = (s) => String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));

  const S = { info: null, list: null, accounts: null, view: 'games', filter: 'all', q: '', loading: false, busy: false, lang: 'en', T: {}, update: null, updateHidden: false };

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
  function toast(msg, err) {
    const el = document.createElement('div');
    el.className = 'toast' + (err ? ' err' : '');
    el.textContent = msg;
    $('#toasts').appendChild(el);
    setTimeout(() => el.remove(), err ? 7000 : 3500);
  }
  async function call(cmd, args) {
    if (!invoke) throw new Error('Checkpoint must run inside the desktop app (Tauri).');
    return invoke(cmd, args || {});
  }
  function close() { layer.innerHTML = ''; }
  function modal(html) {
    layer.innerHTML = `<div class="scrim" data-scrim="1"><div class="modal" role="dialog" aria-modal="true">${html}</div></div>`;
    const b = layer.querySelector('.modal .primary, .modal .danger, .modal button');
    if (b) b.focus();
  }
  function busyModal(title) {
    modal(`<h3>${esc(title)}</h3><div class="loading"><span class="spin"></span><span id="busyText">${h('common.working')}</span></div>`);
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
  async function loadAccounts() {
    try { S.accounts = await call('list_accounts'); $('#cAcc').textContent = S.accounts.length; }
    catch (e) { S.accounts = []; }
  }

  function renderSide() {
    const i = S.info; if (!i) return;
    const b = i.config.backend;
    const where = b.kind === 'folder' ? b.path : b.kind === 'rclone' ? b.remote : t('side.not_connected');
    $('#sideMeta').innerHTML = `
      <div>${h('side.this_pc')}<br><span class="mono">${esc(i.config.device_name)}</span></div>
      <div>${h('side.storage')}<br><span class="mono">${esc(where)}</span></div>
      <div>${h('side.manifest')}<br><span class="mono">${i.manifest_games ? h('side.manifest_games', { n: i.manifest_games.toLocaleString(locale()) }) : h('side.manifest_missing')}</span></div>`;
  }

  // ---------- views ----------
  function render() {
    document.querySelectorAll('#nav button').forEach((b) => b.classList.toggle('on', b.dataset.view === S.view));
    ({ games: renderGames, accounts: renderAccounts, history: renderHistory, settings: renderSettings })[S.view]();
  }

  function banners() {
    const out = [];
    if (S.update && !S.updateHidden) {
      const u = S.update;
      out.push(`<div class="banner"><p>${h('update.available', { version: u.version, current: u.current })}</p>
        <button class="btn" data-act="updLater">${h('update.later')}</button><button class="btn primary" data-act="updShow">${h('update.whats_new')}</button></div>`);
    }
    (S.pending || []).forEach((m) => {
      const v = { game: m.game_name, from: acctName(m.from), to: acctName(m.to) };
      out.push(`<div class="banner warn"><p>${h('games.pending_move', v)}</p>
        <button class="btn" data-act="migUndo" data-mid="${esc(m.id)}">${h('move.undo_btn')}</button><button class="btn primary" data-act="migKeep" data-mid="${esc(m.id)}">${h('move.keep_btn')}</button></div>`);
    });
    if (S.info && !S.info.manifest_games) {
      out.push(`<div class="banner warn"><p>${h('games.no_manifest')}</p><button class="btn primary" data-act="manifest">${h('games.download_manifest')}</button></div>`);
    }
    if (!backendReady()) {
      out.push(`<div class="banner warn"><p>${h('games.no_backend')}</p><button class="btn" data-view="settings">${h('games.open_settings')}</button></div>`);
    } else if (S.list && S.list.cloud_error) {
      out.push(`<div class="banner bad"><p>${h('games.cloud_error', { detail: S.list.cloud_error })}</p><button class="btn" data-act="reload">${h('common.retry')}</button></div>`);
    }
    return out.join('');
  }

  function statusPill(k, label) { return `<span class="pill s-${k}">${esc(label || t('status.' + k))}</span>`; }

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
      .map((k) => `<button class="chip ${S.filter === k ? 'on' : ''}" data-f="${k}">${h(k === 'all' ? 'games.filter_all' : 'status.' + k)} · ${counts[k] || 0}</button>`).join('');
    const q = S.q.trim().toLowerCase();
    const rows = games
      .filter((g) => (S.filter === 'all' || g.status === S.filter) && (!q || g.name.toLowerCase().includes(q) || String(g.appid).includes(q)))
      .map((g) => {
        const where = g.roots.length ? g.roots.join('\n') : g.has_rules ? t('games.no_save_here') : t('games.not_in_manifest');
        const local = g.local_hash ? `<span class="time">${fmt(g.local_mtime)}<small>${esc(ago(g.local_mtime))} · ${esc(files(g.files))} · ${human(g.bytes)}</small></span>` : '<span class="note">—</span>';
        const cloud = g.cloud ? `<span class="time">${fmt(g.cloud.created_utc)}<small>${h('games.cloud_from', { device: g.cloud.device_name, n: g.versions })}</small></span>` : `<span class="note">${h('games.cloud_none')}</span>`;
        const flags = (g.steamid_bound ? `<span class="flag">${h('games.flag_steamid')}</span>` : '') + (g.running ? `<span class="flag run">${h('games.flag_running')}</span>` : '');
        const dis = !backendReady() || g.running || S.busy ? 'disabled' : '';
        return `<tr>
          <td><div class="gname">${esc(g.name)}</div><div class="gsub" title="${esc(where)}">${esc(where.split('\n')[0])}${g.roots.length > 1 ? ` (+${g.roots.length - 1})` : ''}</div>${flags}</td>
          <td><span class="mono">${g.appid}</span></td>
          <td>${local}</td><td>${cloud}</td>
          <td>${statusPill(g.status)}</td>
          <td><div class="acts">
            <button class="btn sm" data-act="backup" data-id="${g.appid}" ${g.local_hash ? dis : 'disabled'}>${h('common.backup')}</button>
            <button class="btn sm" data-act="restore" data-id="${g.appid}" ${g.cloud ? dis : 'disabled'}>${h('common.restore')}</button>
            ${g.steam_ids && g.steam_ids.length ? `<button class="btn sm" data-act="migrate" data-id="${g.appid}" title="${h('games.move_title')}" ${g.running || S.busy ? 'disabled' : ''}>${h('games.move_btn')}</button>` : ''}
          </div></td></tr>`;
      }).join('');
    pane.innerHTML = `
      <div class="panehead"><h2>${h('games.title', { device: S.info ? S.info.config.device_name : '' })}</h2>
        ${S.loading ? '<span class="spin"></span>' : ''}<span class="spacer"></span>
        <button class="btn" data-act="reload" ${S.loading ? 'disabled' : ''}>${h('games.rescan')}</button>
        <button class="btn" data-act="backupAll" ${backendReady() && !S.busy ? '' : 'disabled'}>${h('games.backup_all')}</button>
        <button class="btn primary" data-act="restoreAll" ${backendReady() && !S.busy ? '' : 'disabled'}>${h('games.restore_all')}</button></div>
      ${banners()}
      <div class="filters">${chips}<span class="spacer"></span><input type="text" id="search" class="search" placeholder="${h('games.search')}" value="${esc(S.q)}"></div>
      <div class="tablewrap"><table>
        <thead><tr><th>${h('games.col_game')}</th><th>AppID</th><th>${h('games.col_local')}</th><th>${h('games.col_cloud')}</th><th>${h('games.col_status')}</th><th style="text-align:right">${h('games.col_actions')}</th></tr></thead>
        <tbody>${rows || `<tr><td colspan="6" class="empty">${h('games.no_match')}</td></tr>`}</tbody></table></div>
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
      <div class="card"><div class="gname">${esc(a.persona_name || a.account_name)} ${a.most_recent ? statusPill('sync', t('accounts.most_recent')) : ''}</div>
        <div class="k">login: ${esc(a.account_name)}</div>
        <div class="k">SteamID64: ${esc(a.steamid64)}</div>
        <div class="k">userdata/${a.account_id} · ${a.has_userdata ? h('accounts.apps', { n: a.userdata_apps }) : h('accounts.no_userdata')}</div>
        <div class="note">${a.offline_mode ? h('accounts.offline') + ' · ' : ''}${h('accounts.last_login', { time: fmt(a.timestamp * 1000) })}</div>
        <div class="note">${S.list ? (games.length ? h('accounts.bound_games', { n: games.length, games: games.map((g) => g.name).join(', ') }) : h('accounts.no_bound_games')) : ''}</div>
        ${games.length ? `<div><button class="btn sm" data-act="migrateAcct" data-sid="${esc(a.steamid64)}">${h('accounts.move_btn')}</button></div>` : ''}</div>`;
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
      <div class="list">${items.map((v) => `<div><span class="mono">${fmt(v.created_utc)}</span><span>${esc(v.game_name)} <span class="note">· ${esc(files(v.files))} · ${human(v.bytes)}</span></span><span class="note">${esc(v.device_name)} (${v.os})</span></div>`).join('') || `<div class="empty" style="display:block">${h('history.empty')}</div>`}</div>
      <p class="note">${h('history.footer', { n: S.info ? S.info.config.keep_versions : 10 })}</p>`;
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
          <div class="row"><input type="text" id="fPath" value="${esc(b.kind === 'folder' ? b.path : '')}" placeholder="G:\\My Drive\\Checkpoint"><button class="btn" data-act="pick">${h('settings.choose')}</button></div>
        </div>
      </div>
      <div class="group"><h3>${h('settings.backup')}</h3>
        <label class="check" for="incCfg"><input type="checkbox" id="incCfg" ${c.include_config ? 'checked' : ''}> <span>${h('settings.include_config')}<br><span class="note">${h('settings.include_config_hint')}</span></span></label>
        <div class="field"><label class="l" for="keep">${h('settings.keep_versions')}</label><input type="number" id="keep" min="1" max="100" value="${c.keep_versions}" style="width:100px"></div>
      </div>
      <div class="group"><h3>${h('settings.manifest')}</h3>
        <p class="note">${S.info.manifest_games ? h('settings.manifest_info', { n: S.info.manifest_games.toLocaleString(locale()), time: fmt(S.info.manifest_updated) }) : h('settings.manifest_missing')} ${h('settings.manifest_source')}</p>
        <div class="row"><button class="btn" data-act="manifest">${h('settings.manifest_update')}</button></div>
      </div>
      <div class="group"><h3>${h('settings.updates')}</h3>
        <p class="note">${h('settings.version', { version: S.info.version })}</p>
        <label class="check" for="chkUpd"><input type="checkbox" id="chkUpd" ${c.check_updates !== false ? 'checked' : ''}> <span>${h('settings.check_updates')}<br><span class="note">${h('settings.check_updates_hint')}</span></span></label>
        <div class="row"><button class="btn" data-act="checkUpd">${h('settings.check_now')}</button></div>
      </div>
      <div class="row"><button class="btn primary" data-act="saveCfg">${h('settings.save')}</button><span class="note">${h('settings.saved_in', { path: S.info.config_dir })}</span></div>
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
           <div class="row"><button class="btn primary" data-act="rinstall" ${st.platform ? '' : 'disabled'}>${h('rclone.install')}</button>
           <span class="note">${h('rclone.install_hint', { platform: st.platform || '' })}</span></div></div>`;
    let remoteLine;
    if (!r) remoteLine = h('rclone.not_connected');
    else if (r.custom_client) remoteLine = h('rclone.connected', { name: r.name, scope: r.scope || '?' });
    else remoteLine = h('rclone.shared_client', { name: r.name });
    const ok = r && r.custom_client;
    const step2 = `<div class="step ${ok ? 'done' : 'todo'}"><b>2. Google Drive</b><span>${remoteLine}</span>
        <div class="field"><label class="l" for="cid">Client ID</label><input type="text" id="cid" placeholder="xxxx.apps.googleusercontent.com" autocomplete="off" spellcheck="false" ${st.found ? '' : 'disabled'}></div>
        <div class="field"><label class="l" for="csec">Client secret</label><input type="password" id="csec" placeholder="GOCSPX-…" autocomplete="off" ${st.found ? '' : 'disabled'}></div>
        <div class="row"><button class="btn ${ok ? '' : 'primary'}" data-act="rconnect" ${st.found ? '' : 'disabled'}>${h(r ? 'rclone.reconnect' : 'rclone.connect')}</button>
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
      keep_versions: Math.max(1, Math.min(100, parseInt($('#keep').value, 10) || 10)),
      language: $('#lang').value,
      check_updates: $('#chkUpd').checked,
    };
  }

  // ---------- backup / restore ----------
  const game = (id) => S.list.games.find((g) => g.appid === id);
  const opName = (kind) => t(kind === 'backup' ? 'common.backup' : 'common.restore');

  function box(title, ms, sub, newer) {
    return `<div class="box ${newer ? 'newer' : ''}"><span class="lbl">${esc(title)}</span><span class="big">${fmt(ms)}</span><span class="note">${esc(sub)}</span></div>`;
  }

  async function doOp(kind, g, opts) {
    busyModal(`${opName(kind)} · ${g.name}`);
    S.busy = true;
    try {
      const r = kind === 'backup'
        ? await call('backup_game', { appid: g.appid, force: !!opts.force })
        : await call('restore_game', { appid: g.appid, versionId: opts.versionId || null, force: !!opts.force, remapFrom: opts.remapFrom || null, remapTo: opts.remapTo || null });
      close(); toast(r.message);
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
    const opts = versions.map((x) => `<option value="${esc(x.id)}" ${x.id === v.id ? 'selected' : ''}>${fmt(x.created_utc)} · ${esc(x.device_name)} · ${esc(files(x.files))} · ${human(x.bytes)}</option>`).join('');
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

  function planFor(kind) {
    return S.list.games.map((g) => {
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
      return { g, act, cls, run };
    });
  }

  function openAll(kind) {
    if (!S.list) return;
    const plan = planFor(kind);
    const n = plan.filter((p) => p.run).length;
    modal(`<h3>${h(kind === 'backup' ? 'games.backup_all' : 'games.restore_all')}</h3>
      <p class="note">${h('all.note')}</p>
      <div class="plan">${plan.map((p) => `<div><span>${esc(p.g.name)}</span><span class="pill ${p.cls}">${h(p.act)}</span></div>`).join('')}</div>
      <div class="mfoot"><button class="btn" data-close="1">${h('common.cancel')}</button><button class="btn primary" data-act="runAll" data-kind="${kind}" ${n ? '' : 'disabled'}>${h('all.run', { n })}</button></div>`);
  }

  async function runAll(kind) {
    const todo = planFor(kind).filter((p) => p.run).map((p) => p.g);
    busyModal(t(kind === 'backup' ? 'games.backup_all' : 'games.restore_all'));
    S.busy = true;
    let ok = 0; const errs = [];
    for (let i = 0; i < todo.length; i++) {
      const g = todo[i];
      const el = $('#busyText'); if (el) el.textContent = `${i + 1}/${todo.length} · ${g.name}`;
      try {
        if (kind === 'backup') await call('backup_game', { appid: g.appid, force: false });
        else await call('restore_game', { appid: g.appid, versionId: null, force: false });
        ok++;
      } catch (e) { errs.push(`${g.name}: ${String(e).startsWith('NEEDS_CONFIRM') ? t('all.changed_skipped') : e}`); }
    }
    S.busy = false; close();
    toast(t('all.done', { ok, n: todo.length }));
    if (errs.length) fail(t('err.all_failed', { op: t(kind === 'backup' ? 'games.backup_all' : 'games.restore_all'), n: errs.length }), errs.join('\n'));
    await loadGames();
  }

  // ---------- events ----------
  document.addEventListener('click', async (e) => {
    if (e.target.dataset && e.target.dataset.scrim) { close(); return; }
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
      else if (a === 'migrate') openMigrate({ appid: id });
      else if (a === 'migrateAcct') openMigrate({ from: n.dataset.sid });
      else if (a === 'wzBack') { W.step = 1; renderWizard(); }
      else if (a === 'wzNext') wizardPrepare();
      else if (a === 'wzApply') wizardApply();
      else if (a === 'migKeep' || a === 'migUndo') migDecide(a === 'migKeep' ? 'keep' : 'undo', n.dataset.mid);
      else if (a === 'backupAll') openAll('backup');
      else if (a === 'restoreAll') openAll('restore');
      else if (a === 'runAll') runAll(n.dataset.kind);
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
      else if (a === 'reportBug') reportIssue('bug', '[Bug] ', null);
      else if (a === 'reportFeature') reportIssue('feature', '[Feature] ', null);
      else if (a === 'issueFromError') reportIssue('bug', `[Bug] ${S.lastError ? S.lastError.title : ''}`, S.lastError ? S.lastError.msg : null);
      else if (a === 'openLogs') await call('open_log_folder');
      else if (a === 'copyLog') copyLog();
    } catch (err) { fail(t('err.generic'), err); }
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
      <div class="mfoot"><button class="btn" data-act="openLogs">${h('report.open_logs')}</button><button class="btn" data-act="copyLog">${h('report.copy_log')}</button><span class="spacer"></span>
        <button class="btn" data-close="1">${h('common.close')}</button><button class="btn primary" data-act="issueFromError">${h('report.create_issue')}</button></div>`);
  }

  function openReport() {
    const v = S.info ? S.info.version : '';
    modal(`<h3>${h('nav.report')}</h3>
      <p>${h('report.intro')}</p>
      <div class="cards">
        <div class="card"><b>${h('report.bug')}</b><span class="note">${h('report.bug_hint')}</span><div><button class="btn primary" data-act="reportBug">${h('report.bug')}</button></div></div>
        <div class="card"><b>${h('report.feature')}</b><span class="note">${h('report.feature_hint')}</span><div><button class="btn" data-act="reportFeature">${h('report.feature_btn')}</button></div></div>
      </div>
      <p class="note">${h('report.log_location', { path: S.info ? S.info.log_dir : '', version: v })}</p>
      <div class="mfoot"><button class="btn" data-act="openLogs">${h('report.open_logs')}</button><button class="btn" data-act="copyLog">${h('report.copy_log')}</button><span class="spacer"></span><button class="btn" data-close="1">${h('common.close')}</button></div>`);
  }

  async function reportIssue(kind, title, error) {
    try {
      const r = await call('report_issue', { kind, title, error });
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
      ? `<button class="btn primary" data-act="updInstall" ${S.busy ? 'disabled' : ''}>${h('update.install')}</button>`
      : `<button class="btn primary" data-act="updPage">${h('update.download')}</button>`;
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
      <div class="row"><button class="btn primary" data-act="copyAuth">${h('rclone.copy_link')}</button>
      <button class="btn" data-act="openAuth">${h('rclone.open_link')}</button></div>`;
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
  document.addEventListener('keydown', (e) => { if (e.key === 'Escape' && !S.busy) close(); });

  // ---------- boot ----------
  (async function boot() {
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

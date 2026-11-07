(function () {
  'use strict';
  const invoke = window.__TAURI__ ? window.__TAURI__.core.invoke : null;
  const $ = (s) => document.querySelector(s);
  const pane = $('#pane'), layer = $('#layer');
  const esc = (s) => String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));

  const S = { info: null, list: null, accounts: null, view: 'games', filter: 'all', q: '', loading: false, busy: false };

  const STATUS = {
    sync: 'Đã đồng bộ', local: 'Local mới hơn', cloud: 'Cloud mới hơn', conflict: 'Xung đột',
    diverged: 'Khác nhau', new: 'Chưa backup', empty: 'Không có save', unknown: 'Chưa rõ vị trí save',
  };

  // ---------- helpers ----------
  function fmt(ms) {
    if (!ms) return '—';
    const d = new Date(ms), p = (n) => String(n).padStart(2, '0');
    return `${p(d.getDate())}/${p(d.getMonth() + 1)}/${d.getFullYear()} ${p(d.getHours())}:${p(d.getMinutes())}`;
  }
  function ago(ms) {
    if (!ms) return '';
    const m = Math.round((Date.now() - ms) / 60000);
    if (m < 1) return 'vừa xong';
    if (m < 60) return `${m} phút trước`;
    const h = Math.round(m / 60); if (h < 48) return `${h} giờ trước`;
    return `${Math.round(h / 24)} ngày trước`;
  }
  function diff(ms) {
    const m = Math.round(Math.abs(ms) / 60000), h = Math.floor(m / 60), d = Math.floor(h / 24);
    if (d >= 2) return `${d} ngày`;
    return h ? `${h} giờ ${m % 60} phút` : `${m} phút`;
  }
  function human(b) {
    if (!b) return '0 B';
    if (b >= 1e9) return (b / 1e9).toFixed(2) + ' GB';
    if (b >= 1e6) return (b / 1e6).toFixed(1) + ' MB';
    if (b >= 1e3) return Math.round(b / 1e3) + ' KB';
    return b + ' B';
  }
  function toast(msg, err) {
    const t = document.createElement('div');
    t.className = 'toast' + (err ? ' err' : '');
    t.textContent = msg;
    $('#toasts').appendChild(t);
    setTimeout(() => t.remove(), err ? 7000 : 3500);
  }
  async function call(cmd, args) {
    if (!invoke) throw new Error('Chỉ chạy được trong app Checkpoint (Tauri).');
    return invoke(cmd, args || {});
  }
  function close() { layer.innerHTML = ''; }
  function modal(html) {
    layer.innerHTML = `<div class="scrim" data-scrim="1"><div class="modal" role="dialog" aria-modal="true">${html}</div></div>`;
    const b = layer.querySelector('.modal .primary, .modal .danger, .modal button');
    if (b) b.focus();
  }
  function busyModal(title) {
    modal(`<h3>${esc(title)}</h3><div class="loading"><span class="spin"></span><span id="busyText">Đang chạy…</span></div>`);
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
    catch (e) { S.list = null; fail('Không quét được game', e); }
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
    const where = b.kind === 'folder' ? b.path : b.kind === 'rclone' ? b.remote : 'Chưa kết nối';
    $('#sideMeta').innerHTML = `
      <div>Máy này<br><span class="mono">${esc(i.config.device_name)}</span></div>
      <div>Nơi lưu<br><span class="mono">${esc(where)}</span></div>
      <div>Manifest<br><span class="mono">${i.manifest_games ? i.manifest_games.toLocaleString('vi') + ' game' : 'chưa tải'}</span></div>`;
  }

  // ---------- views ----------
  function render() {
    document.querySelectorAll('#nav button').forEach((b) => b.classList.toggle('on', b.dataset.view === S.view));
    ({ games: renderGames, accounts: renderAccounts, history: renderHistory, settings: renderSettings })[S.view]();
  }

  function banners() {
    const out = [];
    (S.pending || []).forEach((m) => {
      out.push(`<div class="banner warn"><p><b>${esc(m.game_name)}: đã chuyển save từ ${esc(acctName(m.from))} sang ${esc(acctName(m.to))}, chờ mày thử.</b> Mở game bằng ${esc(acctName(m.to))}, vào Continue / Load. Save lên đúng thì Giữ, không thì Hoàn tác.</p>
        <button class="btn" data-act="migUndo" data-mid="${esc(m.id)}">Không được, hoàn tác</button><button class="btn primary" data-act="migKeep" data-mid="${esc(m.id)}">Chạy được, giữ</button></div>`);
    });
    if (S.info && !S.info.manifest_games) {
      out.push(`<div class="banner warn"><p><b>Chưa có manifest vị trí save.</b> Không có nó thì chỉ tìm được save trong thư mục Steam Cloud (userdata). Tải một lần, khoảng vài chục MB.</p><button class="btn primary" data-act="manifest">Tải manifest</button></div>`);
    }
    if (!backendReady()) {
      out.push(`<div class="banner warn"><p><b>Chưa chọn nơi lưu backup.</b> Kết nối Google Drive trong Cài đặt trước khi backup.</p><button class="btn" data-view="settings">Mở Cài đặt</button></div>`);
    } else if (S.list && S.list.cloud_error) {
      out.push(`<div class="banner bad"><p><b>Không đọc được cloud.</b> ${esc(S.list.cloud_error)}</p><button class="btn" data-act="reload">Thử lại</button></div>`);
    }
    return out.join('');
  }

  function statusPill(k) { return `<span class="pill s-${k}">${STATUS[k] || k}</span>`; }

  function renderGames() {
    if (S.loading && !S.list) {
      pane.innerHTML = `<div class="panehead"><h2>Game</h2></div>${banners()}<div class="loading"><span class="spin"></span>Đang quét game và save trên máy…</div>`;
      return;
    }
    const games = S.list ? S.list.games : [];
    const counts = { all: games.length };
    games.forEach((g) => { counts[g.status] = (counts[g.status] || 0) + 1; });
    const chips = [['all', 'Tất cả'], ['local', 'Local mới hơn'], ['cloud', 'Cloud mới hơn'], ['conflict', 'Xung đột'], ['diverged', 'Khác nhau'], ['new', 'Chưa backup'], ['sync', 'Đã đồng bộ'], ['empty', 'Không có save'], ['unknown', 'Chưa rõ']]
      .filter(([k]) => k === 'all' || counts[k])
      .map(([k, l]) => `<button class="chip ${S.filter === k ? 'on' : ''}" data-f="${k}">${l} · ${counts[k] || 0}</button>`).join('');
    const q = S.q.trim().toLowerCase();
    const rows = games
      .filter((g) => (S.filter === 'all' || g.status === S.filter) && (!q || g.name.toLowerCase().includes(q) || String(g.appid).includes(q)))
      .map((g) => {
        const where = g.roots.length ? g.roots.join('\n') : g.has_rules ? 'Không thấy file save trên máy này' : 'Game không có trong manifest';
        const local = g.local_hash ? `<span class="time">${fmt(g.local_mtime)}<small>${ago(g.local_mtime)} · ${g.files} file · ${human(g.bytes)}</small></span>` : '<span class="note">—</span>';
        const cloud = g.cloud ? `<span class="time">${fmt(g.cloud.created_utc)}<small>từ ${esc(g.cloud.device_name)} · ${g.versions} bản</small></span>` : '<span class="note">chưa có</span>';
        const flags = (g.steamid_bound ? '<span class="flag">Save gắn SteamID</span>' : '') + (g.running ? '<span class="flag run">Đang chạy</span>' : '');
        const dis = !backendReady() || g.running || S.busy ? 'disabled' : '';
        return `<tr>
          <td><div class="gname">${esc(g.name)}</div><div class="gsub" title="${esc(where)}">${esc(where.split('\n')[0])}${g.roots.length > 1 ? ` (+${g.roots.length - 1})` : ''}</div>${flags}</td>
          <td><span class="mono">${g.appid}</span></td>
          <td>${local}</td><td>${cloud}</td>
          <td>${statusPill(g.status)}</td>
          <td><div class="acts">
            <button class="btn sm" data-act="backup" data-id="${g.appid}" ${g.local_hash ? dis : 'disabled'}>Backup</button>
            <button class="btn sm" data-act="restore" data-id="${g.appid}" ${g.cloud ? dis : 'disabled'}>Restore</button>
            ${g.steam_ids && g.steam_ids.length ? `<button class="btn sm" data-act="migrate" data-id="${g.appid}" title="Chuyển save sang tài khoản Steam khác" ${g.running || S.busy ? 'disabled' : ''}>Đổi tài khoản</button>` : ''}
          </div></td></tr>`;
      }).join('');
    pane.innerHTML = `
      <div class="panehead"><h2>Game trên ${esc(S.info ? S.info.config.device_name : '')}</h2>
        ${S.loading ? '<span class="spin"></span>' : ''}<span class="spacer"></span>
        <button class="btn" data-act="reload" ${S.loading ? 'disabled' : ''}>Quét lại</button>
        <button class="btn" data-act="backupAll" ${backendReady() && !S.busy ? '' : 'disabled'}>Backup toàn bộ</button>
        <button class="btn primary" data-act="restoreAll" ${backendReady() && !S.busy ? '' : 'disabled'}>Restore toàn bộ</button></div>
      ${banners()}
      <div class="filters">${chips}<span class="spacer"></span><input type="text" id="search" class="search" placeholder="Tìm game hoặc AppID" value="${esc(S.q)}"></div>
      <div class="tablewrap"><table>
        <thead><tr><th>Game · vị trí save</th><th>AppID</th><th>Local</th><th>Cloud mới nhất</th><th>Trạng thái</th><th style="text-align:right">Thao tác</th></tr></thead>
        <tbody>${rows || '<tr><td colspan="6" class="empty">Không có game nào khớp.</td></tr>'}</tbody></table></div>
      <p class="note">Steam: <span class="mono">${esc(S.list ? S.list.steam_root : '')}</span>. Trạng thái so bằng hash nội dung, không dựa vào ngày sửa file.</p>`;
    const s = $('#search');
    if (s) s.addEventListener('input', (e) => {
      S.q = e.target.value; const pos = e.target.selectionStart; renderGames();
      const n = $('#search'); n.focus(); n.setSelectionRange(pos, pos);
    });
  }

  async function renderAccounts() {
    pane.innerHTML = `<div class="panehead"><h2>Tài khoản Steam</h2></div><div class="loading"><span class="spin"></span>Đang đọc…</div>`;
    if (!S.accounts) await loadAccounts();
    if (S.view !== 'accounts') return;
    const cards = (S.accounts || []).map((a) => {
      const games = gamesOfAccount(a.steamid64);
      return `
      <div class="card"><div class="gname">${esc(a.persona_name || a.account_name)} ${a.most_recent ? statusPill('sync').replace(STATUS.sync, 'Đăng nhập gần nhất') : ''}</div>
        <div class="k">login: ${esc(a.account_name)}</div>
        <div class="k">SteamID64: ${esc(a.steamid64)}</div>
        <div class="k">userdata/${a.account_id} ${a.has_userdata ? `· ${a.userdata_apps} app` : '· chưa có'}</div>
        <div class="note">${a.offline_mode ? 'Đang để offline mode · ' : ''}Lần đăng nhập: ${fmt(a.timestamp * 1000)}</div>
        <div class="note">${S.list ? (games.length ? `Có save gắn tài khoản ở ${games.length} game: ${esc(games.map((g) => g.name).join(', '))}` : 'Không có save nào gắn riêng với tài khoản này') : ''}</div>
        ${games.length ? `<div><button class="btn sm" data-act="migrateAcct" data-sid="${esc(a.steamid64)}">Chuyển save sang tài khoản khác…</button></div>` : ''}</div>`;
    }).join('');
    pane.innerHTML = `<div class="panehead"><h2>Tài khoản Steam</h2></div>
      <div class="cards">${cards || '<div class="empty">Không thấy tài khoản nào trong loginusers.vdf</div>'}</div>
      <p class="note">Đọc từ <span class="mono">config/loginusers.vdf</span>. Save của game gắn SteamID chỉ nên restore cho đúng account đã tạo ra nó.</p>`;
  }

  async function renderHistory() {
    pane.innerHTML = `<div class="panehead"><h2>Lịch sử backup</h2></div><div class="loading"><span class="spin"></span>Đang đọc từ cloud…</div>`;
    let items = [];
    try { items = backendReady() ? await call('history') : []; } catch (e) { fail('Không đọc được lịch sử từ cloud', e); }
    if (S.view !== 'history') return;
    pane.innerHTML = `<div class="panehead"><h2>Lịch sử backup</h2></div>
      <div class="list">${items.map((v) => `<div><span class="mono">${fmt(v.created_utc)}</span><span>${esc(v.game_name)} <span class="note">· ${v.files} file · ${human(v.bytes)}</span></span><span class="note">${esc(v.device_name)} (${v.os})</span></div>`).join('') || '<div class="empty" style="display:block">Chưa có bản backup nào.</div>'}</div>
      <p class="note">Mỗi game giữ ${S.info ? S.info.config.keep_versions : 10} bản gần nhất. Restore bản cũ trong hộp thoại Restore của từng game.</p>`;
  }

  function renderSettings() {
    const c = S.info.config, b = c.backend;
    pane.innerHTML = `<div class="panehead"><h2>Cài đặt</h2></div>
    <div class="form">
      <div class="group"><h3>Máy này</h3>
        <div class="field"><label class="l" for="devName">Tên máy</label>
          <input type="text" id="devName" value="${esc(c.device_name)}"><span class="note">Hiện trong lịch sử để biết bản backup đến từ máy nào. ID máy: <span class="mono">${esc(c.device_id)}</span></span></div>
        <div class="field"><label class="l" for="steamRoot">Thư mục Steam</label>
          <input type="text" id="steamRoot" value="${esc(c.steam_root_override || '')}" placeholder="${esc(S.info.steam_root || 'Không tự tìm thấy')}"><span class="note">Để trống để tự tìm.</span></div>
      </div>
      <div class="group"><h3>Nơi lưu backup</h3>
        <div class="field"><label class="l" for="bk">Kiểu</label>
          <select id="bk">
            <option value="none" ${b.kind === 'none' ? 'selected' : ''}>Chưa chọn</option>
            <option value="rclone" ${b.kind === 'rclone' ? 'selected' : ''}>Google Drive qua rclone (Windows + Bazzite)</option>
            <option value="folder" ${b.kind === 'folder' ? 'selected' : ''}>Thư mục (Google Drive for Desktop, ổ mạng…)</option>
          </select></div>
        <div id="bkRclone" ${b.kind === 'folder' ? 'hidden' : ''} class="field">
          <div id="rcPanel" class="steps"><div class="loading"><span class="spin"></span>Đang kiểm tra rclone…</div></div>
          <details class="adv"><summary>Nâng cao</summary>
            <div class="field"><label class="l" for="rRemote">Remote</label>
              <input type="text" id="rRemote" value="${esc(b.kind === 'rclone' ? b.remote : 'gdrive:Checkpoint')}"></div>
            <div class="field"><label class="l" for="rBin">File rclone</label>
              <input type="text" id="rBin" value="${esc(b.kind === 'rclone' ? (b.bin || '') : '')}" placeholder="Để trống để app tự tìm"></div>
          </details>
        </div>
        <div id="bkFolder" ${b.kind === 'folder' ? '' : 'hidden'} class="field">
          <label class="l" for="fPath">Thư mục</label>
          <div class="row"><input type="text" id="fPath" value="${esc(b.kind === 'folder' ? b.path : '')}" placeholder="G:\\My Drive\\Checkpoint"><button class="btn" data-act="pick">Chọn…</button></div>
        </div>
      </div>
      <div class="group"><h3>Backup</h3>
        <label class="check" for="incCfg"><input type="checkbox" id="incCfg" ${c.include_config ? 'checked' : ''}> <span>Backup cả file cài đặt (đồ họa, phím)<br><span class="note">Tắt nếu các máy khác GPU hoặc màn hình, để không bị đè độ phân giải.</span></span></label>
        <div class="field"><label class="l" for="keep">Số bản giữ lại mỗi game</label><input type="number" id="keep" min="1" max="100" value="${c.keep_versions}" style="width:100px"></div>
      </div>
      <div class="group"><h3>Manifest vị trí save</h3>
        <p class="note">${S.info.manifest_games ? `${S.info.manifest_games.toLocaleString('vi')} game có Steam AppID · cập nhật ${fmt(S.info.manifest_updated)}` : 'Chưa tải.'} Nguồn: ludusavi-manifest (PCGamingWiki).</p>
        <div class="row"><button class="btn" data-act="manifest">Cập nhật manifest</button></div>
      </div>
      <div class="row"><button class="btn primary" data-act="saveCfg">Lưu cài đặt</button><span class="note">Cấu hình lưu ở <span class="mono">${esc(S.info.config_dir)}</span></span></div>
    </div>`;
    $('#bk').addEventListener('change', (e) => {
      $('#bkRclone').hidden = e.target.value === 'folder';
      $('#bkFolder').hidden = e.target.value !== 'folder';
    });
    loadRclone();
  }

  // ---------- tài khoản ----------
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

  // ---------- chuyển save sang tài khoản khác ----------
  let W = null;
  const CLASS = {
    none: ['Không có save của tài khoản này', 's-new'],
    folder: ['ID chỉ ở tên thư mục', 's-sync'],
    in_file: ['ID nằm trong file · thử nghiệm', 's-local'],
    opaque: ['File mã hóa · phải thử trong game', 's-local'],
  };

  function openMigrate(opts) {
    if (!S.list) { toast('Đợi quét game xong rồi thử lại.', true); return; }
    const pool = opts.appid ? [game(opts.appid)] : S.list.games;
    const sids = [...new Set(pool.flatMap((g) => (g.steam_ids || []).map(toSid)))];
    if (!sids.length) { toast('Không thấy save nào gắn với tài khoản Steam.', true); return; }
    const from = opts.from && sids.includes(opts.from) ? opts.from : sids[0];
    W = { step: 1, sids, from, to: null, only: opts.appid || null, picked: null, previews: [], results: [], errors: [] };
    renderWizard();
  }

  function wizSteps() {
    return `<div class="wsteps">${['Chọn', 'Chuẩn bị', 'Thử trong game'].map((t, i) => `<span class="${W.step === i + 1 ? 'on' : ''}">${i + 1} · ${t}</span>`).join('')}</div>`;
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
    const toOpts = targets.map((a) => `<option value="${esc(a.steamid64)}" ${a.steamid64 === W.to ? 'selected' : ''}>${esc(acctName(a.steamid64))}${a.most_recent ? ' · đang đăng nhập' : ''}</option>`).join('');
    const list = games.map((g) => `<label class="grow" for="wg${g.appid}"><input type="checkbox" id="wg${g.appid}" data-wg="${g.appid}" ${W.picked.has(g.appid) ? 'checked' : ''}>
        <span><b>${esc(g.name)}</b><br><span class="note">${g.files} file · ${human(g.bytes)}${g.cloud ? ` · backup gần nhất ${fmt(g.cloud.created_utc)}` : ' · chưa backup lên cloud'}</span></span></label>`).join('');
    modal(`<h3>Chuyển save sang tài khoản khác</h3>${wizSteps()}
      <div class="acctpick">
        <div class="field"><label class="l" for="wFrom">Từ tài khoản (bản gốc giữ nguyên)</label><select id="wFrom">${fromOpts}</select></div>
        <span class="arrow" aria-hidden="true">→</span>
        <div class="field"><label class="l" for="wTo">Sang tài khoản</label>${targets.length ? `<select id="wTo">${toOpts}</select>` : '<span class="note">Máy này chưa có tài khoản nào khác. Đăng nhập Steam bằng tài khoản mới 1 lần để nó hiện ở đây.</span>'}</div>
      </div>
      <div class="glist">${list || '<div class="empty">Tài khoản này không có save gắn riêng ở game nào.</div>'}</div>
      <p class="note">Chỉ copy file của tài khoản nguồn sang chỗ của tài khoản đích. Không xóa hay sửa gì của tài khoản nguồn.</p>
      <div class="mfoot"><button class="btn" data-close="1">Hủy</button><button class="btn primary" data-act="wzNext" ${targets.length && games.length ? '' : 'disabled'}>Tiếp</button></div>`);
    $('#wFrom').addEventListener('change', (e) => { W.from = e.target.value; W.picked = null; wizardStep1(); });
    const t = $('#wTo'); if (t) t.addEventListener('change', (e) => { W.to = e.target.value; });
    layer.querySelectorAll('[data-wg]').forEach((c) => c.addEventListener('change', () => {
      const id = +c.dataset.wg; if (c.checked) W.picked.add(id); else W.picked.delete(id);
    }));
  }

  async function wizardPrepare() {
    const ids = [...W.picked];
    if (!ids.length) { toast('Chọn ít nhất 1 game.', true); return; }
    busyModal('Chuyển save sang tài khoản khác');
    W.previews = [];
    for (const appid of ids) {
      try { W.previews.push(await call('migrate_preview', { appid, from: W.from, to: W.to })); }
      catch (e) { W.previews.push({ appid, name: game(appid).name, error: String(e) }); }
    }
    W.step = 2; renderWizard();
  }

  function blockedReason(p) {
    if (p.error) return p.error;
    if (p.running) return 'Game đang chạy. Tắt game trước.';
    if (p.pending_id) return 'Còn một lần chuyển save chưa xác nhận. Giữ hoặc Hoàn tác trước.';
    if (p.class === 'none') return 'Không có file nào của tài khoản nguồn.';
    return null;
  }

  function wizardStep2() {
    const cards = W.previews.map((p) => {
      const blocked = blockedReason(p);
      const g = game(p.appid);
      const [cl, cc] = CLASS[p.class] || ['', ''];
      const ck = (ok, text) => `<div><span class="ck ${ok ? 'ok' : 'todo'}">${ok ? '✓' : '!'}</span><span>${text}</span></div>`;
      const checks = blocked ? `<div class="alert bad">${esc(blocked)}</div>` : `<div class="checklist">
          ${ck(true, 'Game không chạy')}
          ${g && g.cloud ? ck(true, `Đã có bản backup trên cloud lúc ${fmt(g.cloud.created_utc)}`) : ck(false, '<b>Chưa có bản backup trên cloud.</b> Nên Backup game này trước để chắc ăn.')}
          ${p.target_dir_exists ? ck(true, `${esc(acctName(W.to))} đã có thư mục save của game`) : ck(false, `<b>${esc(acctName(W.to))} chưa mở game này lần nào.</b> Nên đăng nhập tài khoản này, vào game tới màn hình chính rồi tắt, để chắc tài khoản có quyền chơi. Bỏ qua cũng được, app tự tạo thư mục.`)}
          ${p.overwrite ? ck(false, `<b>${esc(acctName(W.to))} đã có ${p.overwrite} file save sẽ bị ghi đè.</b> Các file này được chụp lại, hoàn tác sẽ trả về.`) : ''}
          ${p.class === 'opaque' ? ck(false, `File save bị mã hóa hoặc nén (entropy ${p.max_entropy.toFixed(3)} trên tối đa 8). Không biết game có kiểm tra tài khoản bên trong không, phải mở game thử.`) : ''}
          ${p.class === 'in_file' ? `<label class="check" for="patch${p.appid}"><input type="checkbox" id="patch${p.appid}" checked> <span>Thay SteamID bên trong file save (${p.files.filter((f) => f.id_hits).length} file có ID). Thử nghiệm: game có checksum thì vẫn có thể báo hỏng.</span></label>` : ''}
        </div>`;
      const f = p.files && p.files[0];
      const map = f ? `<div class="pathmap"><span class="del">  ${esc(f.from)}</span>\n<span class="add">→ ${esc(f.to)}</span>${p.files.length > 1 ? `\n<span class="del">  … và ${p.files.length - 1} file khác</span>` : ''}</div>` : '';
      return `<div class="step ${blocked ? 'todo' : 'done'}"><div class="row"><b>${esc(p.name)}</b><span class="spacer"></span>${cl ? `<span class="pill ${cc}">${cl}</span>` : ''}</div>${checks}${map}</div>`;
    }).join('');
    const ready = W.previews.filter((p) => !blockedReason(p)).length;
    modal(`<h3>Chuyển save: ${esc(acctName(W.from))} → ${esc(acctName(W.to))}</h3>${wizSteps()}
      <div class="steps">${cards}</div>
      <div class="mfoot"><button class="btn" data-act="wzBack">Quay lại</button><button class="btn primary" data-act="wzApply" ${ready ? '' : 'disabled'}>Chuyển save ${ready} game</button></div>`);
  }

  async function wizardApply() {
    const todo = W.previews.filter((p) => !blockedReason(p));
    const patch = {};
    todo.forEach((p) => { const c = $('#patch' + p.appid); patch[p.appid] = !!(c && c.checked); });
    busyModal('Đang chuyển save');
    S.busy = true;
    W.results = []; W.errors = [];
    for (const p of todo) {
      const t = $('#busyText'); if (t) t.textContent = p.name;
      try { W.results.push(await call('migrate_apply', { appid: p.appid, from: W.from, to: W.to, patchInFile: patch[p.appid] })); }
      catch (e) { W.errors.push(`${p.name}: ${e}`); }
    }
    S.busy = false;
    W.step = 3; W.done = {};
    renderWizard();
    loadGames();
  }

  function wizardStep3() {
    const rows = W.results.map((m) => {
      const d = W.done[m.id];
      const right = d === 'keep' ? '<span class="pill s-sync">Đã giữ</span>'
        : d === 'undo' ? '<span class="pill s-new">Đã hoàn tác</span>'
        : `<button class="btn sm" data-act="migUndo" data-mid="${esc(m.id)}">Không được, hoàn tác</button><button class="btn sm primary" data-act="migKeep" data-mid="${esc(m.id)}">Chạy được, giữ</button>`;
      return `<div class="row mrow2"><span><b>${esc(m.game_name)}</b><br><span class="note">${m.files} file${m.patched ? ` · đã thay ID trong ${m.patched} file` : ''}</span></span><span class="spacer"></span>${right}</div>`;
    }).join('');
    const pending = W.results.some((m) => !W.done[m.id]);
    modal(`<h3>Thử trong game</h3>${wizSteps()}
      ${W.results.length ? `<div class="alert"><b>Đã copy save sang ${esc(acctName(W.to))}.</b><span>Đăng nhập Steam bằng ${esc(acctName(W.to))}, mở từng game, chọn Continue / Load. Save lên đúng tiến trình thì Giữ. Game báo save hỏng hoặc vào New Game thì Hoàn tác: bản copy bị xóa, save cũ của ${esc(acctName(W.to))} (nếu có) được trả lại.</span></div>` : ''}
      ${W.errors.map((e) => `<div class="alert bad">${esc(e)}</div>`).join('')}
      <div class="glist">${rows}</div>
      <p class="note">Có thể đóng hộp thoại này để mở game. Màn hình Game sẽ nhắc cho tới khi mày chọn Giữ hoặc Hoàn tác. Khi Giữ, app nhớ ${esc(acctName(W.from))} → ${esc(acctName(W.to))} trên cloud để máy khác restore tự ghi vào tài khoản mới.</p>
      <div class="mfoot"><button class="btn ${pending ? '' : 'primary'}" data-close="1">${pending ? 'Đóng, để thử game' : 'Xong'}</button></div>`);
  }

  async function migDecide(kind, id) {
    const inWizard = W && W.step === 3 && W.results.some((m) => m.id === id);
    if (!inWizard) busyModal(kind === 'keep' ? 'Giữ kết quả' : 'Hoàn tác');
    try {
      if (kind === 'keep') {
        const r = await call('migrate_keep', { id });
        toast(`${r.migration.game_name}: đã giữ. Lần sau backup sẽ có save của tài khoản mới.`);
        if (r.warning) toast(r.warning, true);
      } else {
        const m = await call('migrate_undo', { id });
        toast(`${m.game_name}: đã hoàn tác, trả lại như trước khi chuyển.`);
      }
      if (inWizard) { W.done[id] = kind; renderWizard(); } else close();
    } catch (e) { fail(kind === 'keep' ? 'Không giữ được kết quả chuyển save' : 'Không hoàn tác được', e); }
    loadGames();
  }

  const OS_NAME = { windows: 'Windows', linux: 'Linux', macos: 'macOS' };

  async function loadRclone() {
    let st;
    try { st = await call('rclone_status'); } catch (e) { st = null; }
    const panel = $('#rcPanel');
    if (!panel || S.view !== 'settings') return;
    if (!st) { panel.innerHTML = '<p class="note">Không kiểm tra được rclone.</p>'; return; }
    const r = st.remote;
    const step1 = st.found
      ? `<div class="step done"><b>1. rclone</b><span>Đã có ${esc(st.version)}${st.managed ? ' (app tự cài)' : ''}</span><span class="note mono">${esc(st.bin)}</span></div>`
      : `<div class="step todo"><b>1. rclone</b><span>Chưa có rclone trên máy này.</span>
           <div class="row"><button class="btn primary" data-act="rinstall" ${st.platform ? '' : 'disabled'}>Cài rclone</button>
           <span class="note">Tải bản chính thức ${esc(st.platform || '')} từ downloads.rclone.org, kiểm tra SHA256, cài vào thư mục của app. Không cần quyền admin.</span></div></div>`;
    let remoteLine;
    if (!r) remoteLine = 'Chưa kết nối.';
    else if (r.custom_client) remoteLine = `Đã kết nối remote <span class="mono">${esc(r.name)}</span> bằng client ID riêng${r.scope ? `, quyền <span class="mono">${esc(r.scope)}</span>` : ''}.`;
    else remoteLine = `Remote <span class="mono">${esc(r.name)}</span> đang dùng client ID chung của rclone: rất chậm và sắp bị Google ngừng. Nhập client ID riêng để kết nối lại.`;
    const ok = r && r.custom_client;
    const step2 = `<div class="step ${ok ? 'done' : 'todo'}"><b>2. Google Drive</b><span>${remoteLine}</span>
        <div class="field"><label class="l" for="cid">Client ID</label><input type="text" id="cid" placeholder="xxxx.apps.googleusercontent.com" autocomplete="off" spellcheck="false" ${st.found ? '' : 'disabled'}></div>
        <div class="field"><label class="l" for="csec">Client secret</label><input type="password" id="csec" placeholder="GOCSPX-…" autocomplete="off" ${st.found ? '' : 'disabled'}></div>
        <div class="row"><button class="btn ${ok ? '' : 'primary'}" data-act="rconnect" ${st.found ? '' : 'disabled'}>${r ? 'Kết nối lại Google Drive' : 'Kết nối Google Drive'}</button>
        <span class="note">Mở trình duyệt để đăng nhập Google. Client secret chỉ lưu trong file cấu hình của rclone trên máy này. Dùng cùng client ID và tài khoản Google trên cả 3 máy.</span></div></div>`;
    panel.innerHTML = `<p class="note">Máy này: ${esc(OS_NAME[st.os] || st.os)} ${esc(st.arch)}</p>${step1}${step2}`;
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
    };
  }

  // ---------- backup / restore ----------
  const game = (id) => S.list.games.find((g) => g.appid === id);

  function box(title, ms, sub, newer) {
    return `<div class="box ${newer ? 'newer' : ''}"><span class="lbl">${esc(title)}</span><span class="big">${fmt(ms)}</span><span class="note">${esc(sub)}</span></div>`;
  }

  async function doOp(kind, g, opts) {
    busyModal(`${kind === 'backup' ? 'Backup' : 'Restore'} · ${g.name}`);
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
      fail(`${kind === 'backup' ? 'Backup' : 'Restore'} ${g.name} không thành công`, msg); render();
    }
  }

  function startBackup(g) {
    if (['cloud', 'conflict', 'diverged'].includes(g.status)) return confirmBackup(g);
    doOp('backup', g, {});
  }

  function confirmBackup(g) {
    const c = g.cloud;
    const text = g.status === 'conflict'
      ? `<b>Xung đột.</b> Cả save trên máy này lẫn bản cloud (từ ${esc(c.device_name)}) đều đã đổi từ lần đồng bộ trước.`
      : g.status === 'diverged'
        ? `<b>Máy này chưa đồng bộ game này lần nào</b>, và save local khác bản cloud. Kiểm tra ngày trước khi đè.`
        : `<b>Cloud đang mới hơn.</b> Bản cloud từ ${esc(c.device_name)} có tiến trình mà máy này chưa có.`;
    modal(`<h3>Backup · ${esc(g.name)}</h3>
      <div class="cmp">${box('Local · máy này', g.local_mtime, ago(g.local_mtime), g.local_mtime > c.created_utc)}${box('Cloud · ' + c.device_name, c.created_utc, ago(c.created_utc), c.created_utc >= g.local_mtime)}</div>
      <div class="alert ${g.status === 'conflict' ? 'bad' : 'warn'}"><span>${text}</span><span>Backup bây giờ sẽ biến bản local thành bản mới nhất. Bản cloud cũ vẫn còn trong lịch sử.</span></div>
      <div class="mfoot"><button class="btn" data-close="1">Hủy</button>
        <button class="btn primary" data-act="toRestore" data-id="${g.appid}">Xem bản cloud để restore</button>
        <button class="btn danger" data-act="forceBackup" data-id="${g.appid}">Vẫn backup</button></div>`);
  }

  async function openRestore(g) {
    busyModal(`Restore · ${g.name}`);
    let versions = [];
    try { versions = await call('list_versions', { appid: g.appid }); } catch (e) { return fail('Không đọc được danh sách bản backup', e); }
    if (!versions.length) { close(); return toast('Chưa có bản backup nào của game này.', true); }
    renderRestore(g, versions, versions[0].id);
  }

  function renderRestore(g, versions, selId) {
    const v = versions.find((x) => x.id === selId) || versions[0];
    const latest = versions[0];
    const hasLocal = !!g.local_hash;
    const same = hasLocal && g.local_hash === v.content_hash;
    const localNewer = hasLocal && g.local_mtime > v.created_utc;
    let alert = '';
    if (same) alert = `<div class="alert">Bản local giống hệt bản này (cùng hash). Restore không thay đổi gì.</div>`;
    else if (!hasLocal) alert = `<div class="alert">Máy này chưa có save của game. Restore an toàn.</div>`;
    else if (g.status === 'conflict') alert = `<div class="alert bad"><b>Xung đột: cả hai bên đều đã đổi từ lần đồng bộ trước.</b><span>Save trên máy này được chơi tiếp sau lần đồng bộ, bản cloud cũng có thêm tiến trình từ ${esc(latest.device_name)}. Không gộp được, phải chọn một.</span></div>`;
    else if (g.status === 'local' || localNewer) alert = `<div class="alert warn"><b>Bản local mới hơn bản này ${diff(g.local_mtime - v.created_utc)}.</b><span>Restore sẽ ghi đè tiến trình trên máy này. Có lẽ mày muốn Backup thay vì Restore.</span></div>`;
    else if (g.status === 'diverged') alert = `<div class="alert warn"><b>Máy này chưa đồng bộ game này lần nào</b><span>và save local khác bản cloud. Bản cloud mới hơn ${diff(v.created_utc - g.local_mtime)} theo ngày sửa file.</span></div>`;
    else alert = `<div class="alert">Bản cloud mới hơn, từ ${esc(v.device_name)}. Restore an toàn.</div>`;
    if (v.id !== latest.id) alert += `<div class="alert warn">Đây là bản cũ, không phải bản mới nhất (${fmt(latest.created_utc)}).</div>`;
    // Bản backup gắn với đúng 1 tài khoản: cho chọn ghi vào tài khoản khác
    const verAccts = [...new Set((v.steam_ids || []).map(toSid))];
    let acctPick = '';
    if (verAccts.length === 1) {
      const src = verAccts[0];
      const alias = S.list && S.list.account_aliases ? S.list.account_aliases[src] : null;
      const cur = (S.accounts || []).find((a) => a.most_recent);
      const def = alias || src;
      const others = (S.accounts || []).filter((a) => a.steamid64 !== src);
      const opt = (sid, label) => `<option value="${esc(sid)}" ${sid === def ? 'selected' : ''}>${esc(label)}</option>`;
      acctPick = `<div class="field"><label class="l" for="remapTo">Ghi save vào tài khoản</label>
        <select id="remapTo" data-from="${esc(src)}">${opt(src, `${acctName(src)} (như bản gốc)`)}${others.map((a) => opt(a.steamid64, `${acctName(a.steamid64)}${a.most_recent ? ' · đang đăng nhập' : ''}`)).join('')}</select>
        <span class="note">${alias ? `Đã nhớ: ${esc(acctName(src))} được thay bằng ${esc(acctName(alias))}.` : cur && cur.steamid64 !== src ? `Bản này của ${esc(acctName(src))}, máy đang đăng nhập ${esc(acctName(cur.steamid64))}. Chọn tài khoản đang đăng nhập để chơi tiếp bằng nó (game mã hóa save có thể không nhận).` : 'Save gắn với tài khoản Steam. Chỉ đổi khi muốn chơi tiếp bằng tài khoản khác.'}</span></div>`;
    } else if (v.steam_ids && v.steam_ids.length) {
      alert += `<div class="alert">Save gắn với SteamID/AccountID <span class="mono">${esc(v.steam_ids.join(', '))}</span>. Chỉ chơi được trên đúng account đó.</div>`;
    }
    if (S.info.os === 'linux' && v.os === 'windows') alert += `<div class="alert">Bản từ Windows sẽ được ghi vào Proton prefix của game. Nếu chưa chạy game lần nào trên máy này thì chạy 1 lần trước.</div>`;
    const danger = hasLocal && !same && (g.status === 'local' || g.status === 'conflict' || g.status === 'diverged' || localNewer);
    const opts = versions.map((x) => `<option value="${esc(x.id)}" ${x.id === v.id ? 'selected' : ''}>${fmt(x.created_utc)} · ${esc(x.device_name)} · ${x.files} file · ${human(x.bytes)}</option>`).join('');
    modal(`<h3>Restore · ${esc(g.name)}</h3>
      <div class="field"><label class="l" for="ver">Phiên bản trên cloud</label><select id="ver">${opts}</select></div>
      <div class="cmp">${box('Local · máy này', g.local_mtime, hasLocal ? `${ago(g.local_mtime)} · ${g.files} file` : 'không có save', localNewer)}${box('Cloud · ' + v.device_name, v.created_utc, `${ago(v.created_utc)} · ${v.os}`, !localNewer && !same && hasLocal)}</div>
      ${alert}
      ${acctPick}
      ${hasLocal && !same ? '<p class="note">Bản local hiện tại sẽ được chụp snapshot trước khi ghi đè.</p>' : ''}
      <div class="mfoot"><button class="btn" data-close="1">Hủy</button>
        ${danger ? `<button class="btn primary" data-act="toBackup" data-id="${g.appid}">Backup thay vì restore</button>` : ''}
        <button class="btn ${danger ? 'danger' : 'primary'}" data-act="doRestore" data-id="${g.appid}" data-ver="${esc(v.id)}" ${same ? 'disabled' : ''}>${danger ? 'Vẫn restore, ghi đè local' : 'Restore'}</button></div>`);
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
      if (kind === 'backup') {
        if (g.running) { act = 'Bỏ qua, game đang chạy'; cls = 's-conflict'; }
        else if (g.status === 'local' || g.status === 'new') { act = 'Backup'; cls = 's-local'; run = true; }
        else if (g.status === 'sync') { act = 'Bỏ qua, không đổi'; cls = 's-sync'; }
        else if (['cloud', 'conflict', 'diverged'].includes(g.status)) { act = 'Bỏ qua, cần xử lý tay'; cls = 's-conflict'; }
        else { act = 'Bỏ qua, không có save'; cls = 's-new'; }
      } else {
        if (g.running) { act = 'Bỏ qua, game đang chạy'; cls = 's-conflict'; }
        else if (g.status === 'cloud') { act = 'Restore'; cls = 's-cloud'; run = true; }
        else if (g.status === 'sync') { act = 'Bỏ qua, không đổi'; cls = 's-sync'; }
        else if (['local', 'conflict', 'diverged'].includes(g.status)) { act = 'Bỏ qua, cần xử lý tay'; cls = 's-conflict'; }
        else { act = 'Bỏ qua, không có bản cloud'; cls = 's-new'; }
      }
      return { g, act, cls, run };
    });
  }

  function openAll(kind) {
    if (!S.list) return;
    const plan = planFor(kind);
    const n = plan.filter((p) => p.run).length;
    modal(`<h3>${kind === 'backup' ? 'Backup' : 'Restore'} toàn bộ</h3>
      <p class="note">Game ở trạng thái xung đột hoặc ngược chiều không bị đụng tới. Mở từng game để quyết định.</p>
      <div class="plan">${plan.map((p) => `<div><span>${esc(p.g.name)}</span><span class="pill ${p.cls}">${p.act}</span></div>`).join('')}</div>
      <div class="mfoot"><button class="btn" data-close="1">Hủy</button><button class="btn primary" data-act="runAll" data-kind="${kind}" ${n ? '' : 'disabled'}>Chạy ${n} game</button></div>`);
  }

  async function runAll(kind) {
    const todo = planFor(kind).filter((p) => p.run).map((p) => p.g);
    busyModal(`${kind === 'backup' ? 'Backup' : 'Restore'} toàn bộ`);
    S.busy = true;
    let ok = 0; const errs = [];
    for (let i = 0; i < todo.length; i++) {
      const g = todo[i];
      const t = $('#busyText'); if (t) t.textContent = `${i + 1}/${todo.length} · ${g.name}`;
      try {
        if (kind === 'backup') await call('backup_game', { appid: g.appid, force: false });
        else await call('restore_game', { appid: g.appid, versionId: null, force: false });
        ok++;
      } catch (e) { errs.push(`${g.name}: ${String(e).startsWith('NEEDS_CONFIRM') ? 'trạng thái đã đổi, bỏ qua' : e}`); }
    }
    S.busy = false; close();
    toast(`Xong ${ok}/${todo.length} game.`);
    if (errs.length) fail(`${kind === 'backup' ? 'Backup' : 'Restore'} toàn bộ: ${errs.length} game lỗi`, errs.join('\n'));
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
        busyModal('Tải manifest vị trí save');
        try { const c = await call('update_manifest'); close(); toast(`Đã tải manifest: ${c.toLocaleString('vi')} game.`); await loadInfo(); await loadGames(); }
        catch (err) { fail('Không tải được manifest', err); }
      }
      else if (a === 'pick') { const p = await call('pick_folder'); if (p) $('#fPath').value = p; }
      else if (a === 'rinstall') {
        busyModal('Cài rclone');
        $('#busyText').textContent = 'Đang tải rclone (khoảng 25 MB) và kiểm tra SHA256…';
        S.busy = true;
        try { const v = await call('rclone_install'); close(); toast(`Đã cài ${v}`); loadRclone(); }
        catch (err) { fail('Không cài được rclone', err); }
        S.busy = false;
      }
      else if (a === 'rconnect') {
        const clientId = $('#cid').value.trim(), clientSecret = $('#csec').value.trim();
        if (!clientId || !clientSecret) { toast('Nhập cả Client ID và Client secret.', true); return; }
        busyModal('Kết nối Google Drive');
        $('#busyText').textContent = 'Đăng nhập Google trong trình duyệt vừa mở, cho phép quyền, rồi quay lại đây. Nếu sau đó trình duyệt báo "127.0.0.1 refused to connect" thì bỏ qua, rclone đã nhận mã và tắt máy chủ tạm.';
        S.busy = true;
        try {
          const r = await call('rclone_connect', { clientId, clientSecret });
          S.busy = false; close(); toast(`Đã kết nối: ${r}`);
          await loadInfo(); S.list = null; render(); loadGames();
        } catch (err) { S.busy = false; fail('Không kết nối được Google Drive', err); loadRclone(); }
      }
      else if (a === 'saveCfg') {
        const cfg = readSettings();
        await call('save_config', { config: cfg });
        await loadInfo(); toast('Đã lưu cài đặt.'); S.list = null; S.accounts = null; render(); loadGames();
      }
      else if (a === 'report') openReport();
      else if (a === 'reportBug') reportIssue('bug', '[Lỗi] ', null);
      else if (a === 'reportFeature') reportIssue('feature', '[Đề xuất] ', null);
      else if (a === 'issueFromError') reportIssue('bug', `[Lỗi] ${S.lastError ? S.lastError.title : ''}`, S.lastError ? S.lastError.msg : null);
      else if (a === 'openLogs') await call('open_log_folder');
      else if (a === 'copyLog') copyLog();
    } catch (err) { fail('Có lỗi xảy ra', err); }
  });

  // ---------- báo lỗi ----------
  function logUi(level, message) {
    if (!invoke) return;
    invoke('log_frontend', { level, message: String(message) }).catch(() => {});
  }
  window.addEventListener('error', (e) => logUi('error', `JS: ${e.message} @ ${e.filename}:${e.lineno}:${e.colno}`));
  window.addEventListener('unhandledrejection', (e) => logUi('error', `Promise: ${e.reason && e.reason.stack ? e.reason.stack : e.reason}`));

  /** Popup lỗi có nút Tạo issue. Backend đã ghi lỗi của lệnh vào log; ở đây ghi thêm ngữ cảnh trên giao diện. */
  function fail(title, e) {
    const msg = String(e && e.message ? e.message : e);
    logUi('error', `${title}: ${msg}`);
    S.lastError = { title, msg };
    S.busy = false;
    modal(`<h3>${esc(title)}</h3>
      <div class="alert bad"><span class="errmsg">${esc(msg)}</span></div>
      <p class="note">Nếu lỗi lặp lại, bấm <b>Tạo issue trên GitHub</b>: trang GitHub mở trong trình duyệt với phiên bản, hệ điều hành, thông báo lỗi và log điền sẵn (đường dẫn thư mục người dùng và SteamID đã được che). Chỉ cần đăng nhập GitHub, xem lại rồi bấm Submit.</p>
      <div class="mfoot"><button class="btn" data-act="openLogs">Mở thư mục log</button><button class="btn" data-act="copyLog">Copy log</button><span class="spacer"></span>
        <button class="btn" data-close="1">Đóng</button><button class="btn primary" data-act="issueFromError">Tạo issue trên GitHub</button></div>`);
  }

  function openReport() {
    const v = S.info ? S.info.version : '';
    modal(`<h3>Báo lỗi / Góp ý</h3>
      <p>Mở trang GitHub của Checkpoint trong trình duyệt. Cần tài khoản GitHub (miễn phí) để gửi.</p>
      <div class="cards">
        <div class="card"><b>Báo lỗi</b><span class="note">App chạy sai, báo lỗi, hay save không như mong đợi. Phiên bản, hệ điều hành và log được điền sẵn (đã che đường dẫn thư mục người dùng và SteamID).</span><div><button class="btn primary" data-act="reportBug">Báo lỗi</button></div></div>
        <div class="card"><b>Đề xuất tính năng</b><span class="note">Ý tưởng để app tiện hơn. Không gửi kèm log.</span><div><button class="btn" data-act="reportFeature">Đề xuất</button></div></div>
      </div>
      <p class="note">Log nằm ở <span class="mono">${esc(S.info ? S.info.log_dir : '')}</span>. Checkpoint ${esc(v)}.</p>
      <div class="mfoot"><button class="btn" data-act="openLogs">Mở thư mục log</button><button class="btn" data-act="copyLog">Copy log</button><span class="spacer"></span><button class="btn" data-close="1">Đóng</button></div>`);
  }

  async function reportIssue(kind, title, error) {
    try {
      const r = await call('report_issue', { kind, title, error });
      toast('Đã mở GitHub trong trình duyệt. Xem lại nội dung rồi bấm Submit.');
      if (r.log_truncated) toast('Log dài nên chỉ điền phần cuối. Cần thêm thì bấm Mở thư mục log và kéo file checkpoint.log vào issue.');
    } catch (e) { toast(String(e), true); }
  }

  async function copyLog() {
    const text = await call('get_log', { lines: 2000 });
    try { await navigator.clipboard.writeText(text); toast('Đã copy log (đã che đường dẫn thư mục người dùng và SteamID).'); }
    catch (e) { toast('Không copy được. Bấm Mở thư mục log và gửi file checkpoint.log.', true); }
  }
  function confirmOrBackup(g) {
    if (g.status === 'local' || g.status === 'new') doOp('backup', g, {});
    else confirmBackup(g);
  }
  document.addEventListener('keydown', (e) => { if (e.key === 'Escape' && !S.busy) close(); });

  // ---------- boot ----------
  (async function boot() {
    try {
      await loadInfo();
      loadAccounts();
      if (!backendReady()) { S.view = 'settings'; render(); toast('Chọn nơi lưu backup trước.'); loadGames(); return; }
      await loadGames();
    } catch (e) {
      pane.innerHTML = `<div class="banner bad"><p>${esc(e)}</p></div>`;
    }
  })();
})();

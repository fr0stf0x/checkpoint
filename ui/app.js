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
    catch (e) { S.list = null; toast(String(e), true); }
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
    const cards = (S.accounts || []).map((a) => `
      <div class="card"><div class="gname">${esc(a.persona_name || a.account_name)} ${a.most_recent ? statusPill('sync').replace(STATUS.sync, 'Đăng nhập gần nhất') : ''}</div>
        <div class="k">login: ${esc(a.account_name)}</div>
        <div class="k">SteamID64: ${esc(a.steamid64)}</div>
        <div class="k">userdata/${a.account_id} ${a.has_userdata ? `· ${a.userdata_apps} app` : '· chưa có'}</div>
        <div class="note">${a.offline_mode ? 'Đang để offline mode · ' : ''}Lần đăng nhập: ${fmt(a.timestamp * 1000)}</div></div>`).join('');
    pane.innerHTML = `<div class="panehead"><h2>Tài khoản Steam</h2></div>
      <div class="cards">${cards || '<div class="empty">Không thấy tài khoản nào trong loginusers.vdf</div>'}</div>
      <p class="note">Đọc từ <span class="mono">config/loginusers.vdf</span>. Save của game gắn SteamID chỉ nên restore cho đúng account đã tạo ra nó.</p>`;
  }

  async function renderHistory() {
    pane.innerHTML = `<div class="panehead"><h2>Lịch sử backup</h2></div><div class="loading"><span class="spin"></span>Đang đọc từ cloud…</div>`;
    let items = [];
    try { items = backendReady() ? await call('history') : []; } catch (e) { toast(String(e), true); }
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
        <div id="bkRclone" ${b.kind === 'rclone' ? '' : 'hidden'} class="field">
          <label class="l" for="rRemote">Remote</label>
          <div class="row"><input type="text" id="rRemote" value="${esc(b.kind === 'rclone' ? b.remote : 'gdrive:Checkpoint')}"></div>
          <label class="l" for="rBin">File rclone</label>
          <div class="row"><input type="text" id="rBin" value="${esc(b.kind === 'rclone' ? (b.bin || '') : '')}" placeholder="rclone (lấy từ PATH)"><button class="btn" data-act="rcheck">Kiểm tra</button></div>
          <div class="row"><button class="btn primary" data-act="rconnect">Kết nối Google Drive</button><span class="note">Mở trình duyệt để đăng nhập Google, tạo remote tên <span class="mono">gdrive</span>. Cả 3 máy dùng chung 1 tài khoản Google là thấy cùng dữ liệu.</span></div>
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
      $('#bkRclone').hidden = e.target.value !== 'rclone';
      $('#bkFolder').hidden = e.target.value !== 'folder';
    });
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
        : await call('restore_game', { appid: g.appid, versionId: opts.versionId || null, force: !!opts.force });
      close(); toast(r.message);
      S.busy = false;
      await loadGames();
    } catch (e) {
      S.busy = false;
      const msg = String(e);
      if (msg.startsWith('NEEDS_CONFIRM')) { close(); return kind === 'backup' ? confirmBackup(g) : openRestore(g); }
      close(); toast(msg, true); render();
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
    try { versions = await call('list_versions', { appid: g.appid }); } catch (e) { close(); return toast(String(e), true); }
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
    if (v.steam_ids && v.steam_ids.length) alert += `<div class="alert">Save gắn với SteamID/AccountID <span class="mono">${esc(v.steam_ids.join(', '))}</span>. Chỉ chơi được trên đúng account đó.</div>`;
    if (S.info.os === 'linux' && v.os === 'windows') alert += `<div class="alert">Bản từ Windows sẽ được ghi vào Proton prefix của game. Nếu chưa chạy game lần nào trên máy này thì chạy 1 lần trước.</div>`;
    const danger = hasLocal && !same && (g.status === 'local' || g.status === 'conflict' || g.status === 'diverged' || localNewer);
    const opts = versions.map((x) => `<option value="${esc(x.id)}" ${x.id === v.id ? 'selected' : ''}>${fmt(x.created_utc)} · ${esc(x.device_name)} · ${x.files} file · ${human(x.bytes)}</option>`).join('');
    modal(`<h3>Restore · ${esc(g.name)}</h3>
      <div class="field"><label class="l" for="ver">Phiên bản trên cloud</label><select id="ver">${opts}</select></div>
      <div class="cmp">${box('Local · máy này', g.local_mtime, hasLocal ? `${ago(g.local_mtime)} · ${g.files} file` : 'không có save', localNewer)}${box('Cloud · ' + v.device_name, v.created_utc, `${ago(v.created_utc)} · ${v.os}`, !localNewer && !same && hasLocal)}</div>
      ${alert}
      ${hasLocal && !same ? '<p class="note">Bản local hiện tại sẽ được chụp snapshot trước khi ghi đè.</p>' : ''}
      <div class="mfoot"><button class="btn" data-close="1">Hủy</button>
        ${danger ? `<button class="btn primary" data-act="toBackup" data-id="${g.appid}">Backup thay vì restore</button>` : ''}
        <button class="btn ${danger ? 'danger' : 'primary'}" data-act="doRestore" data-id="${g.appid}" data-ver="${esc(v.id)}" ${same ? 'disabled' : ''}>${danger ? 'Vẫn restore, ghi đè local' : 'Restore'}</button></div>`);
    $('#ver').addEventListener('change', (e) => renderRestore(g, versions, e.target.value));
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
    errs.forEach((e) => toast(e, true));
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
      else if (a === 'doRestore') doOp('restore', game(id), { versionId: n.dataset.ver, force: true });
      else if (a === 'backupAll') openAll('backup');
      else if (a === 'restoreAll') openAll('restore');
      else if (a === 'runAll') runAll(n.dataset.kind);
      else if (a === 'manifest') {
        busyModal('Tải manifest vị trí save');
        try { const c = await call('update_manifest'); close(); toast(`Đã tải manifest: ${c.toLocaleString('vi')} game.`); await loadInfo(); await loadGames(); }
        catch (err) { close(); toast(String(err), true); }
      }
      else if (a === 'pick') { const p = await call('pick_folder'); if (p) $('#fPath').value = p; }
      else if (a === 'rcheck') { const v = await call('rclone_check', { bin: $('#rBin').value.trim() || null }); toast(v || 'rclone chạy được'); }
      else if (a === 'rconnect') {
        busyModal('Kết nối Google Drive');
        $('#busyText').textContent = 'Đăng nhập Google trong trình duyệt vừa mở, rồi quay lại đây…';
        try { const r = await call('rclone_connect', { name: 'gdrive', bin: (S.info.config.backend.bin) || null }); close(); toast(`Đã kết nối: ${r}`); await loadInfo(); render(); }
        catch (err) { close(); toast(String(err), true); }
      }
      else if (a === 'saveCfg') {
        const cfg = readSettings();
        await call('save_config', { config: cfg });
        await loadInfo(); toast('Đã lưu cài đặt.'); S.list = null; S.accounts = null; render(); loadGames();
      }
    } catch (err) { toast(String(err), true); }
  });
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

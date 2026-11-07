(function () {
  'use strict';
  const REPO = 'fr0stf0x/checkpoint';
  const API = 'https://api.github.com/repos/' + REPO;
  const $ = (id) => document.getElementById(id);
  const fmtNum = (n) => n.toLocaleString('vi-VN');

  // Điền link thật của trang đang chạy (vd. https://ten-du-an.vercel.app) vào phần hướng dẫn Google
  document.querySelectorAll('[data-copy-origin]').forEach((el) => {
    el.textContent = location.origin + el.dataset.copyOrigin;
  });
  document.querySelectorAll('[data-copy-host]').forEach((el) => {
    el.textContent = location.hostname;
  });
  document.querySelectorAll('code.copy').forEach((el) => {
    el.title = 'Bấm để copy';
    el.addEventListener('click', async () => {
      try {
        await navigator.clipboard.writeText(el.textContent);
        el.classList.add('copied');
        setTimeout(() => el.classList.remove('copied'), 1200);
      } catch (e) {
        const r = document.createRange(); r.selectNodeContents(el);
        const s = getSelection(); s.removeAllRanges(); s.addRange(r);
      }
    });
  });

  function visitorOs() {
    const ua = navigator.userAgent;
    if (/Windows/i.test(ua)) return 'windows';
    if (/Linux|X11/i.test(ua) && !/Android/i.test(ua)) return 'linux';
    return 'other';
  }

  async function getJson(url) {
    const r = await fetch(url, { headers: { Accept: 'application/vnd.github+json' } });
    if (!r.ok) throw new Error(r.status);
    return r.json();
  }

  async function load() {
    // Cache 10 phút để không chạm giới hạn 60 request/giờ của GitHub API
    let data = null;
    try {
      const c = JSON.parse(sessionStorage.getItem('gh') || 'null');
      if (c && Date.now() - c.t < 600000) data = c.d;
    } catch (e) { /* bỏ qua */ }
    if (!data) {
      const [repo, releases] = await Promise.all([
        getJson(API),
        getJson(API + '/releases?per_page=100').catch(() => []),
      ]);
      data = {
        stars: repo.stargazers_count,
        releases: releases.filter((r) => !r.draft).map((r) => ({
          tag: r.tag_name, date: r.published_at, pre: r.prerelease,
          assets: r.assets.map((a) => ({ name: a.name, url: a.browser_download_url, n: a.download_count })),
        })),
      };
      try { sessionStorage.setItem('gh', JSON.stringify({ t: Date.now(), d: data })); } catch (e) { /* bỏ qua */ }
    }

    $('stStars').textContent = fmtNum(data.stars);
    const total = data.releases.reduce((s, r) => s + r.assets.reduce((x, a) => x + a.n, 0), 0);
    $('stDl').textContent = fmtNum(total);

    const latest = data.releases.find((r) => !r.pre) || data.releases[0];
    if (!latest) { $('stVer').textContent = 'chưa có'; $('stVerDate').textContent = 'bản phát hành'; return; }
    $('stVer').textContent = latest.tag;
    $('stVerDate').textContent = 'phát hành ' + new Date(latest.date).toLocaleDateString('vi-VN');

    const win = latest.assets.find((a) => /setup\.exe$/i.test(a.name)) || latest.assets.find((a) => /\.(exe|msi)$/i.test(a.name));
    const lin = latest.assets.find((a) => /\.AppImage$/i.test(a.name));
    const main = $('dlMain'), alt = $('dlAlt');
    const os = visitorOs();
    if (os === 'linux' && lin) {
      main.href = lin.url; main.textContent = `Tải cho Linux / Bazzite (${latest.tag})`;
      if (win) { alt.href = win.url; alt.textContent = 'Bản Windows'; }
    } else if (win) {
      main.href = win.url; main.textContent = `Tải cho Windows (${latest.tag})`;
      if (lin) { alt.href = lin.url; alt.textContent = 'Bản Linux / Bazzite'; }
    }
  }

  load().catch(() => {
    document.getElementById('stats').classList.add('offline');
  });
})();

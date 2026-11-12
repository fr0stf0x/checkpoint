(function () {
  'use strict';
  const REPO = 'fr0stf0x/checkpoint';
  const API = 'https://api.github.com/repos/' + REPO;
  const LANGS = ['en', 'vi'];
  const $ = (id) => document.getElementById(id);
  const t = (k, o) => i18next.t(k, o);
  const locale = () => (i18next.resolvedLanguage === 'vi' ? 'vi-VN' : 'en-US');
  const page = document.body.dataset.page || 'home';
  let gh = null; // cached GitHub data, re-rendered on language change

  // ---------- old links ----------
  // The guides used to be sections of the home page; send old anchors to their own pages.
  const MOVED = { '#guide': '/guide', '#google': '/guide/google-drive' };
  if (page === 'home' && MOVED[location.hash]) {
    location.replace(MOVED[location.hash] + location.search);
    return;
  }

  // ---------- icons (same line style as the app) ----------
  const ICONS = {
    pc: '<rect x="3" y="4" width="18" height="12" rx="2"/><path d="M8 20h8M12 16v4"/>',
    handheld: '<rect x="2" y="6" width="20" height="12" rx="4"/><path d="M6 12h3M7.5 10.5v3M16 11h.01M18 13h.01"/><rect x="9.5" y="9" width="5" height="6" rx="1"/>',
    cloud: '<path d="M7 19a5 5 0 1 1 .9-9.9A6 6 0 0 1 19 11a4 4 0 0 1-1 8z"/>',
    upload: '<path d="M7 18a5 5 0 1 1 .9-9.9A6 6 0 0 1 19 10a4 4 0 0 1-1 8"/><path d="M12 12v9M9 15l3-3 3 3"/>',
    download: '<path d="M7 18a5 5 0 1 1 .9-9.9A6 6 0 0 1 19 10a4 4 0 0 1-1 8"/><path d="M12 12v9M9 18l3 3 3-3"/>',
    arrow: '<path d="M4 12h16M14 6l6 6-6 6"/>',
    offline: '<path d="M2 2l20 20M8.5 16.5a5 5 0 0 1 7 0M5 13a10 10 0 0 1 5.2-2.8M19 13a10 10 0 0 0-2-1.5M12 20h.01M1.5 9a15 15 0 0 1 4.2-2.6M14.5 6.2A15 15 0 0 1 22.5 9"/>',
    swap: '<path d="M4 8h15l-4-4M20 16H5l4 4"/>',
    shield: '<path d="M12 3l8 3v6c0 4.5-3.4 8.3-8 9-4.6-.7-8-4.5-8-9V6z"/><path d="m9 12 2 2 4-4"/>',
    lock: '<rect x="4" y="11" width="16" height="10" rx="2"/><path d="M8 11V7a4 4 0 0 1 8 0v4"/>',
    sparkle: '<path d="M12 3l1.8 4.7 4.7 1.8-4.7 1.8L12 16l-1.8-4.7-4.7-1.8 4.7-1.8z"/><path d="M19 15l.8 2.2 2.2.8-2.2.8L19 21l-.8-2.2-2.2-.8 2.2-.8z"/>',
    users: '<circle cx="9" cy="8" r="4"/><path d="M2 21v-1a6 6 0 0 1 6-6h2a6 6 0 0 1 6 6v1M16 4.1a4 4 0 0 1 0 7.8M22 21v-1a6 6 0 0 0-4-5.6"/>',
    chevron: '<path d="m9 6 6 6-6 6"/>',
    book: '<path d="M4 4.5A2.5 2.5 0 0 1 6.5 2H20v16H6.5A2.5 2.5 0 0 0 4 20.5z"/><path d="M4 20.5A2.5 2.5 0 0 0 6.5 23H20v-5"/>',
  };
  const ic = (n) => `<svg class="ic" viewBox="0 0 24 24" aria-hidden="true" focusable="false">${ICONS[n] || ''}</svg>`;

  // ---------- shared header and footer ----------
  const NAV = [['home', '/', 'nav.home'], ['guide', '/guide', 'nav.guide'], ['google', '/guide/google-drive', 'nav.google'], ['account', '/guide/switch-account', 'nav.account']];
  const top = $('top');
  if (top) {
    top.className = 'top';
    top.innerHTML = `<div class="bar"><a class="logo" href="/"><img src="/icon.png" alt="">Checkpoint</a>
      <nav aria-label="Main">${NAV.map(([id, href, key]) => `<a href="${href}" data-i18n="${key}"${id === page ? ' class="on" aria-current="page"' : ''}></a>`).join('')}
        <a href="https://github.com/${REPO}">GitHub</a>
        <a href="#" id="langSwitch" class="lang" data-i18n="nav.switch"></a>
        <a class="dlnav" href="${page === 'home' ? '#top-download' : '/#top-download'}" data-i18n="nav.download"></a></nav></div>`;
  }
  const foot = $('foot');
  if (foot) {
    foot.innerHTML = `<span class="grow" data-i18n="footer.about"></span>
      <a href="/guide" data-i18n="footer.guide"></a>
      <a href="https://github.com/${REPO}">GitHub</a>
      <a href="https://github.com/${REPO}/issues" data-i18n="footer.bug"></a>
      <a href="https://buymeacoffee.com/fr0stf0x">Buy Me a Coffee</a>
      <a href="/privacy" data-i18n="footer.privacy"></a>
      <a href="/terms" data-i18n="footer.terms"></a>`;
  }
  document.querySelectorAll('[data-ic]').forEach((el) => { el.outerHTML = ic(el.dataset.ic); });

  // ---------- i18n ----------
  // Translations are trusted static files (locales/<lang>.json) and may contain markup.
  function applyTranslations() {
    document.documentElement.lang = i18next.resolvedLanguage;
    document.title = t(document.body.dataset.title || 'meta.title');
    document.querySelectorAll('[data-i18n]').forEach((el) => { el.innerHTML = t(el.dataset.i18n); });
    document.querySelectorAll('[data-i18n-attr]').forEach((el) => {
      el.dataset.i18nAttr.split(';').forEach((pair) => {
        const [attr, key] = pair.split(':');
        el.setAttribute(attr, t(key));
      });
    });
    document.querySelectorAll('code.copy').forEach((el) => { el.title = t('copy.title'); });
    renderStats();
  }

  // ---------- copy helpers for the Google Branding values ----------
  document.querySelectorAll('[data-copy-origin]').forEach((el) => {
    el.textContent = location.origin + el.dataset.copyOrigin;
  });
  document.querySelectorAll('[data-copy-host]').forEach((el) => {
    el.textContent = location.hostname;
  });
  document.querySelectorAll('code.copy').forEach((el) => {
    el.tabIndex = 0;
    const copy = async () => {
      try {
        await navigator.clipboard.writeText(el.textContent);
        el.classList.add('copied');
        setTimeout(() => el.classList.remove('copied'), 1200);
      } catch (e) {
        const r = document.createRange(); r.selectNodeContents(el);
        const s = getSelection(); s.removeAllRanges(); s.addRange(r);
      }
    };
    el.addEventListener('click', copy);
    el.addEventListener('keydown', (e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); copy(); } });
  });

  // ---------- GitHub stats and download buttons (home page only) ----------
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

  async function loadGitHub() {
    // Cached for 10 minutes to stay under GitHub's 60 requests/hour limit
    try {
      const c = JSON.parse(sessionStorage.getItem('gh') || 'null');
      if (c && Date.now() - c.t < 600000) return c.d;
    } catch (e) { /* ignore */ }
    const [repo, releases] = await Promise.all([
      getJson(API),
      getJson(API + '/releases?per_page=100').catch(() => []),
    ]);
    const data = {
      stars: repo.stargazers_count,
      releases: releases.filter((r) => !r.draft).map((r) => ({
        tag: r.tag_name, date: r.published_at, pre: r.prerelease,
        assets: r.assets.map((a) => ({ name: a.name, url: a.browser_download_url, n: a.download_count })),
      })),
    };
    try { sessionStorage.setItem('gh', JSON.stringify({ t: Date.now(), d: data })); } catch (e) { /* ignore */ }
    return data;
  }

  function renderStats() {
    if (!gh || !$('stStars')) return;
    const fmtNum = (n) => n.toLocaleString(locale());
    $('stStars').textContent = fmtNum(gh.stars);
    // Every file of every release, the same total as the README's downloads badge (shields.io)
    $('stDl').textContent = fmtNum(gh.releases.reduce((s, r) => s + r.assets.reduce((x, a) => x + a.n, 0), 0));
    const latest = gh.releases.find((r) => !r.pre) || gh.releases[0];
    if (!latest) { $('stVer').textContent = t('stats.none'); $('stVerDate').textContent = t('stats.no_release'); return; }
    $('stVer').textContent = latest.tag;
    $('stVerDate').textContent = t('stats.released', { date: new Date(latest.date).toLocaleDateString(locale()) });

    const win = latest.assets.find((a) => /setup\.exe$/i.test(a.name)) || latest.assets.find((a) => /\.(exe|msi)$/i.test(a.name));
    const lin = latest.assets.find((a) => /\.AppImage$/i.test(a.name));
    const main = $('dlMain'), alt = $('dlAlt');
    if (visitorOs() === 'linux' && lin) {
      main.href = lin.url; main.textContent = t('dl.lin', { tag: latest.tag });
      if (win) { alt.href = win.url; alt.textContent = t('dl.win_alt'); }
    } else if (win) {
      main.href = win.url; main.textContent = t('dl.win', { tag: latest.tag });
      if (lin) { alt.href = lin.url; alt.textContent = t('dl.lin_alt'); }
    }
  }

  // ---------- boot ----------
  $('langSwitch').addEventListener('click', (e) => {
    e.preventDefault();
    const next = i18next.resolvedLanguage === 'vi' ? 'en' : 'vi';
    const url = new URL(location.href);
    url.searchParams.set('lng', next);
    history.replaceState(null, '', url);
    i18next.changeLanguage(next);
  });

  i18next
    .use(i18nextHttpBackend)
    .use(i18nextBrowserLanguageDetector)
    .init({
      supportedLngs: LANGS,
      fallbackLng: 'en',
      load: 'languageOnly',
      backend: { loadPath: '/locales/{{lng}}.json' },
      detection: { order: ['querystring', 'localStorage', 'navigator'], lookupQuerystring: 'lng', caches: ['localStorage'] },
      interpolation: { escapeValue: false },
    })
    .then(applyTranslations);
  i18next.on('languageChanged', () => { if (i18next.isInitialized) applyTranslations(); });

  if ($('stats')) {
    loadGitHub()
      .then((d) => { gh = d; renderStats(); })
      .catch(() => { $('stats').classList.add('offline'); });
  }
})();

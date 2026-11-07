(function () {
  'use strict';
  const REPO = 'fr0stf0x/checkpoint';
  const API = 'https://api.github.com/repos/' + REPO;
  const LANGS = ['en', 'vi'];
  const $ = (id) => document.getElementById(id);
  const t = (k, o) => i18next.t(k, o);
  const locale = () => (i18next.resolvedLanguage === 'vi' ? 'vi-VN' : 'en-US');
  let gh = null; // cached GitHub data, re-rendered on language change

  // ---------- i18n ----------
  // Translations are trusted static files (locales/<lang>.json) and may contain markup.
  function applyTranslations() {
    document.documentElement.lang = i18next.resolvedLanguage;
    document.title = t('meta.title');
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

  // ---------- GitHub stats and download buttons ----------
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
    if (!gh) return;
    const fmtNum = (n) => n.toLocaleString(locale());
    $('stStars').textContent = fmtNum(gh.stars);
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

  loadGitHub()
    .then((d) => { gh = d; renderStats(); })
    .catch(() => { $('stats').classList.add('offline'); });
})();

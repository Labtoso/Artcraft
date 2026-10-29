/* ArtCraft mirror: runs the original website's own scripts (animations, menus, 3D) and takes out
   what doesn't belong to this fork: the AI studio, pricing, community and social links, the
   original's services. Links go to this repository; German comes from i18n/de.json.
   Nothing here touches the page before the original scripts have taken it over (hydration),
   so they keep working; until then the parts that will change are only hidden. */
(function () {
  var C = __CONFIG__;
  var BASE = C.base;
  var html = document.documentElement;

  // ---- addresses -------------------------------------------------------------------------
  function pagePath(p) {
    if (!/\.[a-z0-9]{2,5}$/i.test(p) && p.charAt(p.length - 1) !== '/') p += '/';
    return p;
  }
  function fixUrl(u) {
    var url;
    try { url = new URL(u, location.href); } catch (e) { return u; }
    if (url.origin !== location.origin) return u;
    if (url.pathname !== BASE && url.pathname.indexOf(BASE + '/') !== 0) url.pathname = BASE + pagePath(url.pathname);
    url.searchParams.delete('_rsc');
    return url.pathname + url.search + url.hash;
  }
  ['pushState', 'replaceState'].forEach(function (m) {
    var orig = history[m];
    history[m] = function (s, t, u) { return orig.call(this, s, t, u == null ? u : fixUrl(String(u))); };
  });
  // the original's client-side page changes need a server this static copy doesn't have:
  // every internal link is a plain page load
  window.addEventListener('click', function (e) {
    if (e.defaultPrevented || e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
    var a = e.target && e.target.closest ? e.target.closest('a[href]') : null;
    if (!a || (a.target && a.target !== '_self') || a.hasAttribute('download')) return;
    var href = a.getAttribute('href');
    if (!href || href.charAt(0) === '#') return;
    var u = new URL(a.href, location.href);
    if (u.origin !== location.origin) return;
    if (u.pathname === location.pathname && u.hash) return;
    e.preventDefault();
    e.stopImmediatePropagation();
    location.href = fixUrl(u.href);
  }, true);
  // no background calls to the original's services (prefetches, referral tracking)
  var blocked = /_rsc=|storyteller\.ai\/v1|google-analytics|googletagmanager/;
  if (window.fetch) {
    var ofetch = window.fetch;
    window.fetch = function (input, init) {
      var u = typeof input === 'string' ? input : input ? String(input.url || input.href || input) : '';
      if (blocked.test(u)) return Promise.reject(new TypeError('blocked'));
      return ofetch.apply(this, arguments);
    };
  }
  if (navigator.sendBeacon) {
    var obeacon = navigator.sendBeacon.bind(navigator);
    navigator.sendBeacon = function (u, d) { return blocked.test(String(u)) ? true : obeacon(u, d); };
  }

  // ---- downloads: the original's GitHub files -> this repository's builds ---------------------
  var GH = /^https:\/\/github\.com\/storytold\/([\w.-]+)(?:\/(.*))?$/i;
  function appFor(repo) { repo = repo.toLowerCase(); return repo === 'craft-launcher' ? 'artcraft-launcher' : repo; }
  function kindOf(name) {
    var n = name.toLowerCase();
    var os = /windows|win(32|64)|\.msi$|\.exe$/.test(n) ? 'windows' : /linux|\.appimage$|\.deb$|\.rpm$|\.flatpak$/.test(n) ? 'linux' : /mac|darwin|\.dmg$/.test(n) ? 'mac' : 'other';
    return os;
  }
  // a download we don't build (macOS, ARM, Flatpak ...): null
  function mapDownload(repo, tag, name) {
    var app = C.apps[appFor(repo)];
    if (!app) return C.repo + '/releases/tag/suite';
    var ver = tag.replace(/^v/i, '');
    var want = name.split(ver).join(app.version);
    if (app.assets[want]) return app.assets[want];
    return null;
  }
  function fallbackDownload(repo, name) {
    var app = C.apps[appFor(repo)];
    if (!app) return C.repo + '/releases/tag/suite';
    var os = kindOf(name), names = Object.keys(app.assets);
    var pick = os === 'linux' ? /\.AppImage$/ : os === 'windows' ? /x64\.msi$|\.msi$/ : null;
    for (var i = 0; pick && i < names.length; i++) if (pick.test(names[i])) return app.assets[names[i]];
    return app.release;
  }
  function mapGitHub(href) {
    var m = GH.exec(href);
    if (!m) return href.replace(/^https:\/\/github\.com\/storytold\/?$/i, C.repo + '/releases');
    var repo = m[1], rest = m[2] || '';
    var app = appFor(repo);
    var d = /^releases\/download\/([^/]+)\/([^/?#]+)/.exec(rest);
    if (d) return mapDownload(repo, d[1], decodeURIComponent(d[2]));
    if (/^releases/.test(rest)) return C.apps[app] ? C.apps[app].release : C.repo + '/releases/tag/suite';
    if (/^issues/.test(rest)) return C.repo + '/issues';
    // an app's (or the launcher's) code on the original -> its builds here; anything else -> all releases
    return C.apps[app] ? C.apps[app].release : C.repo + '/releases';
  }

  // ---- what goes ---------------------------------------------------------------------------
  var GONE_LINK = /app\.getartcraft\.com|discord\.(gg|com)|bsky\.app|bluesky|twitter\.com|\/\/(www\.)?x\.com\/|linkedin\.com|instagram\.com|tiktok\.com|youtube\.com\/@|reddit\.com\/(submit|r\/)|facebook\.com\/sharer|t\.me\/share|^mailto:|^tel:/i;
  var GONE_PAGE = /^\/(pricing|tutorials|news|faq)(\/|$)/i;
  var GONE_LABEL = /^(artcraft studio|community|pricing)$/i;
  function hide(el) { if (el && !el.hasAttribute('data-ac-hide')) el.setAttribute('data-ac-hide', ''); }
  // the smallest wrapper of a link that holds nothing else (list item, card)
  function linkBox(a) {
    var box = a;
    while (box.parentElement && /^(LI|SPAN)$/.test(box.parentElement.tagName) && box.parentElement.querySelectorAll('a,button').length === 1) box = box.parentElement;
    return box;
  }
  function own(el) { return el.closest('[data-ac-own]'); }

  // whole sections that go; they also lose their id, so the original's section index skips them
  var PAGE = location.pathname.indexOf(BASE) === 0 ? pagePath(location.pathname.slice(BASE.length) || '/') : '/';
  html.setAttribute('data-ac-page', PAGE);
  var GONE_IDS = { '*': ['studio', 'features', 'ownership', 'made-with', 'pricing'], '/press-kit/': ['articles', 'contact'], '/apps/': ['start'] };
  function goneSections(root) {
    var ids = GONE_IDS['*'].concat(GONE_IDS[PAGE] || []);
    // an app's page ends with "Help shape ...": the original's Discord
    if (/^\/apps\/[^/]+\/$/.test(PAGE)) ids.push('start');
    ids.forEach(function (id) {
      var el = document.getElementById(id);
      if (el && el.tagName === 'SECTION') { hide(el); el.setAttribute('data-ac-id', id); el.removeAttribute('id'); }
    });
    if (PAGE === '/download/') {
      var secs = document.querySelectorAll('main > section');
      if (secs[1] && !secs[1].id && secs[1].querySelector('figure')) hide(secs[1]);
    }
  }

  function fixLink(a) {
    if (own(a)) return;
    var href = a.getAttribute('href');
    if (!href || a.getAttribute('data-ac-href') === href) return;
    var out = href;
    if (href.charAt(0) === '/' && href.charAt(1) !== '/') {
      var path = href.split(/[?#]/)[0];
      if (GONE_PAGE.test(path)) { hide(linkBox(a)); return; }
      out = fixUrl(href);
    } else if (GONE_LINK.test(href)) {
      hide(linkBox(a)); return;
    } else if (/^https:\/\/getartcraft\.com/i.test(href)) {
      out = fixUrl(href.replace(/^https:\/\/getartcraft\.com/i, ''));
    } else if (GH.test(href) || /^https:\/\/github\.com\/storytold\/?$/i.test(href)) {
      out = mapGitHub(href);
      if (out === null) {
        // a file we don't build: menu entries go, single buttons get the closest build
        if (a.closest('li')) { hide(a.closest('li')); return; }
        var m = GH.exec(href);
        out = fallbackDownload(m[1], href.split('/').pop());
      }
      if (a.hasAttribute('download')) a.setAttribute('download', out.split('/').pop());
    }
    if (out !== href) a.setAttribute('href', out);
    a.setAttribute('data-ac-href', out);
  }

  function hideByLabel(root) {
    // footer columns and cards titled "ArtCraft Studio" / "Community"
    root.querySelectorAll('nav > p.hud-label, article p.hud-label, div > p.hud-label').forEach(function (p) {
      var t = p.textContent.replace(/\s+/g, ' ').trim();
      if (!GONE_LABEL.test(t)) return;
      var box = p.closest('nav') || p.closest('article');
      if (box && !box.querySelector('a[href*="/apps"]')) hide(box);
    });
    root.querySelectorAll('p').forEach(function (p) {
      if (/^\s*Looking for ArtCraft Studio\?/.test(p.textContent)) hide(p);
    });
  }

  // ---- texts that change -------------------------------------------------------------------
  // [selector, test, new text]; applied once the original scripts own the page
  var TEXT = [
    ['p', /^ArtCraft makes two things/, 'Thirteen native, open-source desktop apps. ArtCraft Launcher installs them all and keeps them up to date.'],
    ['p', /^Open-source tools for artists: the Craft Apps and ArtCraft Studio/, 'Open-source tools for artists: the Craft Apps.'],
    ['p', /^Find answers, join the community/, 'Report a problem or ask a question on GitHub.'],
    ['p', /the world's only open source precision AI tool for artists/, null],
    ['p', /^Tell us what to rebuild next/, 'New Craft Apps show up here by themselves as soon as the original team releases them.'],
    ['p', /^Free for every platform above\. Questions or feedback\?/, 'Free for every platform above. Questions or problems? Open an issue on GitHub.'],
    ['*', /^Launch App$/i, 'Download'],
    // no Flatpak build here: the Linux button gives the AppImage
    ['span', /^For Linux · Flatpak/, function (t) { return t.replace('Flatpak', 'AppImage'); }]
  ];
  function setOnlyText(el, text) {
    var nodes = [], w = document.createTreeWalker(el, NodeFilter.SHOW_TEXT, null), n;
    while ((n = w.nextNode())) if (n.nodeValue.trim()) nodes.push(n);
    if (!nodes.length) return;
    nodes[0].nodeValue = text;
    for (var i = 1; i < nodes.length; i++) nodes[i].nodeValue = '';
  }
  function texts(root) {
    TEXT.forEach(function (r) {
      root.querySelectorAll(r[0] === '*' ? 'a,button,span' : r[0]).forEach(function (el) {
        if (el.hasAttribute('data-ac-text') || own(el)) return;
        var t = el.textContent.replace(/\s+/g, ' ').trim();
        if (!r[1].test(t)) return;
        if (r[0] === '*' && el.children.length > 1) return;
        if (typeof r[2] === 'function') return setOnlyText(el, r[2](t));
        if (r[2] === null) setOnlyText(el, t.replace("the world's only open source precision AI tool for artists", 'the Crafting Apps'));
        else setOnlyText(el, r[2]);
        el.setAttribute('data-ac-text', '');
      });
    });
    // copyright
    // (React splits "© 2026 ArtCraft" into separate text nodes)
    root.querySelectorAll('footer p, footer span, footer div').forEach(function (el) {
      if (!/^©\s*\d{4}\s*ArtCraft$/.test(el.textContent.replace(/\s+/g, ' ').trim())) return;
      el.childNodes.forEach(function (n) {
        if (n.nodeType === 3 && /ArtCraft/.test(n.nodeValue)) n.nodeValue = n.nodeValue.replace('ArtCraft', 'LabtosoStudios');
      });
    });
    downloadPage();
    noMac(root);
    linuxColumn(root);
  }

  // the download page is about ArtCraft Studio on the original; here it is about the launcher
  var ON_LINUX = /linux/i.test(navigator.userAgent) && !/android/i.test(navigator.userAgent);
  var DL_TEXT = {
    'ArtCraft Studio · macOS · Windows · Web': 'ArtCraft Launcher · Windows · Linux',
    'Download ArtCraft ': 'Download the Craft ',
    'The generative AI studio for images and video. Edit on the canvas, compose 3D scenes and render them with the latest models, right on your desktop.':
      'ArtCraft Launcher installs all thirteen open-source Craft Apps with one click and keeps them up to date, right on your desktop.',
    'Universal macOS build · x64 Windows': 'x64 Windows · x86_64 Linux',
    'macOS': 'Linux',
    'macOS 12.0 or newer': 'Ubuntu 22.04, Fedora 38 or newer (x86_64)',
    'Install & sign in': 'Install',
    'Create a free account or log in.': 'Start ArtCraft Launcher and pick your apps.',
    'Start generating artwork immediately.': 'Every app keeps itself up to date.'
  };
  function launcherFile(re) {
    var app = C.apps['artcraft-launcher'];
    if (!app) return C.repo + '/releases/tag/suite';
    var names = Object.keys(app.assets);
    for (var i = 0; i < names.length; i++) if (re.test(names[i])) return app.assets[names[i]];
    return app.release;
  }
  // pick a system and a package, then download the file straight from this repository's release
  var PACKAGES = {
    windows: [
      [/windows-x64\.msi$/, 'Installer (.msi)', 'Installationsprogramm (.msi)'],
      [/windows-x64-portable\.zip$/, 'Portable (.zip)', 'Portabel (.zip)']
    ],
    linux: [
      [/x86_64\.AppImage$/, 'AppImage (any distribution)', 'AppImage (jede Distribution)'],
      [/x86_64\.deb$/, 'Debian, Ubuntu, Mint (.deb)', 'Debian, Ubuntu, Mint (.deb)'],
      [/x86_64\.rpm$/, 'Fedora, openSUSE, RHEL (.rpm)', 'Fedora, openSUSE, RHEL (.rpm)'],
      [/x86_64\.tar\.gz$/, 'Tarball (.tar.gz)', 'Archiv (.tar.gz)']
    ]
  };
  var chosen = { os: ON_LINUX ? 'linux' : 'windows', pkg: 0 };
  function chooser(after) {
    var app = C.apps['artcraft-launcher'];
    if (!app || !after.parentElement) return;
    var box = after.parentElement.querySelector('[data-ac-own="chooser"]');
    if (!box) {
      box = document.createElement('div');
      box.setAttribute('data-ac-own', 'chooser');
      box.setAttribute('data-ac-skip', '');
      box.className = 'ac-chooser';
      after.parentElement.insertBefore(box, after.nextSibling);
      box._cls = after.className;
    }
    var de = lang === 'de', key = chosen.os + chosen.pkg + lang;
    if (box.getAttribute('data-ac-state') === key) return;
    box.setAttribute('data-ac-state', key);
    var names = Object.keys(app.assets);
    function file(re) { for (var i = 0; i < names.length; i++) if (re.test(names[i])) return names[i]; return null; }
    var list = PACKAGES[chosen.os].filter(function (p) { return file(p[0]); });
    if (chosen.pkg >= list.length) chosen.pkg = 0;
    box.textContent = '';
    var tabs = document.createElement('div');
    tabs.className = 'ac-tabs hud-label';
    [['windows', 'Windows'], ['linux', 'Linux']].forEach(function (o) {
      var b = document.createElement('button');
      b.type = 'button';
      b.textContent = o[1];
      if (o[0] === chosen.os) b.setAttribute('aria-pressed', 'true');
      b.addEventListener('click', function () { chosen.os = o[0]; chosen.pkg = 0; chooser(after); });
      tabs.appendChild(b);
    });
    box.appendChild(tabs);
    if (list.length > 1) {
      var sel = document.createElement('select');
      sel.className = 'ac-pkg';
      sel.setAttribute('aria-label', de ? 'Paket' : 'Package');
      list.forEach(function (p, i) {
        var o = document.createElement('option');
        o.value = String(i);
        o.textContent = de ? p[2] : p[1];
        if (i === chosen.pkg) o.selected = true;
        sel.appendChild(o);
      });
      sel.addEventListener('change', function () { chosen.pkg = +sel.value; chooser(after); });
      box.appendChild(sel);
    }
    var pick = list[chosen.pkg], name = pick && file(pick[0]);
    var a = document.createElement('a');
    a.className = box._cls;
    a.href = name ? app.assets[name] : app.release;
    if (name) a.setAttribute('download', name);
    a.textContent = (de ? 'Herunterladen für ' : 'Download for ') + (chosen.os === 'linux' ? 'Linux' : 'Windows');
    box.appendChild(a);
    var info = document.createElement('p');
    info.className = 'hud-label ac-file';
    info.textContent = name || '';
    box.appendChild(info);
  }

  function downloadPage() {
    var main = document.querySelector('main');
    if (PAGE !== '/download/' || !main) return;
    var w = document.createTreeWalker(main, NodeFilter.SHOW_TEXT, null), n, launcher = C.apps['artcraft-launcher'];
    while ((n = w.nextNode())) {
      var v = n.nodeValue, e = n.parentElement;
      if (DL_TEXT.hasOwnProperty(v)) n.nodeValue = DL_TEXT[v];
      else if (v === 'Studio' && e.closest('h1')) n.nodeValue = 'Apps';
      else if (launcher && v !== launcher.version && /^\d+\.\d+\.\d+$/.test(v) && /Open source/.test(e.textContent)) n.nodeValue = launcher.version;
    }
    main.querySelectorAll('h2').forEach(function (h) {
      if (!/^Looking for the Crafting Apps\?$/.test(h.textContent.trim())) return;
      var box = h;
      while (box.parentElement && !box.parentElement.querySelector('h1')) box = box.parentElement;
      hide(box);
    });
    // the buttons: Windows -> the launcher's installer, Mac -> Linux (AppImage); the big one follows the visitor's system
    main.querySelectorAll('a').forEach(function (a) {
      if (own(a)) return;
      var t = a.textContent.replace(/\s+/g, ' ').trim();
      if (!/^Download for (Windows|Mac|Linux)$/.test(t)) return;
      var card = a.parentElement;
      while (card && !card.querySelector('h1,h2')) card = card.parentElement;
      var h2 = card && !card.querySelector('h1') && card.querySelectorAll('h2').length === 1 ? card.querySelector('h2') : null;
      var inCard = !!h2 && /^(Windows|Linux|macOS)$/.test(h2.textContent.trim());
      var linux = inCard ? h2.textContent.trim() !== 'Windows' : ON_LINUX;
      var os = linux ? 'Linux' : 'Windows', tw = document.createTreeWalker(a, NodeFilter.SHOW_TEXT, null), c;
      while ((c = tw.nextNode())) if (/^(Windows|Mac|Linux)$/.test(c.nodeValue) && c.nodeValue !== os) c.nodeValue = os;
      var out = launcherFile(linux ? /\.AppImage$/ : /\.msi$/);
      if (a.getAttribute('href') !== out) { a.setAttribute('data-ac-href', out); a.setAttribute('href', out); }
      if (a.hasAttribute('download')) a.removeAttribute('download');
      // the big button gives way to the chooser below
      if (!inCard) { hide(a); chooser(a); }
    });
    main.querySelectorAll('h2').forEach(function (h) {
      if (h.textContent.trim() !== 'Linux') return;
      var box = h.parentElement;
      box.querySelectorAll('svg').forEach(function (s) { if (!s.closest('a,li')) hide(s.parentElement !== box && s.parentElement.children.length === 1 ? s.parentElement : s); });
    });
  }

  // there are no macOS builds here: the macOS download column and the macOS chips go
  function noMac(root) {
    root.querySelectorAll('article h3').forEach(function (h) {
      if (h.textContent.trim() === 'macOS') hide(h.closest('article'));
    });
    root.querySelectorAll('div,span').forEach(function (e) {
      if (e.children.length || e.textContent.trim() !== 'macOS' || !e.parentElement || e.parentElement.children.length < 2) return;
      if (e.closest('h1,h2,h3,a')) return;
      hide(e);
    });
    var w = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, null), n;
    while ((n = w.nextNode())) if (/^macOS · /.test(n.nodeValue)) n.nodeValue = n.nodeValue.replace(/^macOS · /, '');
  }

  // an app's Linux column leads with Flatpak on the original; here the AppImage is the main download
  function linuxColumn(root) {
    if (!/^\/apps\/[^/]+\/$/.test(PAGE)) return;
    root.querySelectorAll('article h3').forEach(function (h) {
      if (h.textContent.trim() !== 'Linux') return;
      var art = h.closest('article'), lead = false;
      var w = document.createTreeWalker(art, NodeFilter.SHOW_TEXT, null), n;
      while ((n = w.nextNode())) {
        if (n.parentElement.closest('li')) continue;
        if (n.nodeValue === 'Flatpak (x86_64)') { n.nodeValue = 'AppImage (x86_64)'; lead = true; }
        else if (/^Sandboxed\. Install with flatpak/.test(n.nodeValue)) n.nodeValue = 'Any distro. Mark it executable and run.';
        else if (n.nodeValue === 'AppImage (x86_64)') lead = true;
      }
      if (!lead) return;
      art.querySelectorAll('li').forEach(function (li) {
        if (/\.AppImage$/i.test(li.textContent.trim()) && /x86_64/.test(li.textContent)) hide(li);
      });
    });
  }

  // the top-right button: "Download" to the download page, like on the original
  function headerButton() {
    var nav = document.querySelector('nav');
    if (!nav) return;
    nav.querySelectorAll('a').forEach(function (a) {
      if (a.hasAttribute('data-ac-download')) return;
      var t = (a.getAttribute('data-ac-was') || a.textContent).replace(/\s+/g, ' ').trim();
      if (!/^launch app$/i.test(t) && !/^download$/i.test(t)) return;
      if (/^download$/i.test(t) && !a.className.match(/font-mono|uppercase/)) return;
      a.setAttribute('data-ac-was', 'Launch App');
      a.setAttribute('data-ac-download', '');
      a.removeAttribute('data-ac-hide');
      a.setAttribute('href', BASE + '/download/');
      a.setAttribute('data-ac-href', BASE + '/download/');
      a.removeAttribute('target');
      setOnlyText(a, 'Download');
    });
  }

  // ---- what this copy adds ------------------------------------------------------------------
  function addOriginalLink() {
    var nav = document.querySelector('nav');
    var ul = nav && nav.querySelector('ul');
    if (!ul || ul.querySelector('[data-ac-own="original"]')) return;
    var model = null;
    ul.querySelectorAll(':scope > li').forEach(function (li) {
      var a = li.querySelector(':scope > a');
      var span = a && a.querySelector('span');
      if (!model && a && !li.hasAttribute('data-ac-hide') && span && !span.classList.contains('bg-invert-bg') && !li.classList.contains('border-l')) model = li;
    });
    if (!model) return;
    var li = model.cloneNode(true);
    li.setAttribute('data-ac-own', 'original');
    li.classList.add('ml-1.5', 'border-l', 'border-line', 'pl-1.5');
    var a = li.querySelector('a');
    a.setAttribute('href', C.original);
    a.setAttribute('target', '_blank');
    a.setAttribute('rel', 'noopener');
    setOnlyText(a, 'Original on GitHub');
    ul.appendChild(li);
  }
  function addCredits() {
    if (document.querySelector('[data-ac-own="credits"]')) return;
    var footer = document.querySelector('footer');
    if (!footer) return;
    var box = document.createElement('div');
    box.setAttribute('data-ac-own', 'credits');
    box.className = 'mx-auto max-w-[1280px] border-x border-b border-line px-6 py-6 md:px-10 text-sm leading-relaxed text-muted';
    box.innerHTML = C.credits;
    footer.appendChild(box);
  }

  // ---- German ------------------------------------------------------------------------------
  var LKEY = 'artcraft-lang';
  function pref() {
    try { var s = localStorage.getItem(LKEY); if (s) return s; } catch (e) {}
    var langs = navigator.languages || [navigator.language || 'en'];
    return /^de\b/i.test(langs[0] || '') ? 'de' : 'en';
  }
  var lang = pref();
  var DE = null;
  var originals = new WeakMap();
  var applied = new WeakMap();
  function norm(s) { return s.replace(/\s+/g, ' ').trim(); }
  var INLINE = /^(SPAN|A|EM|STRONG|B|I|CODE|SMALL|MARK|U)$/;
  var SKIP_IN = /^(svg|path|rect|line|circle|polyline|polygon|g|img)$/i;
  function orig(n) {
    // the English text: what we put there ourselves doesn't count
    if (applied.has(n) && applied.get(n) === n.nodeValue) return originals.get(n);
    originals.set(n, n.nodeValue);
    return n.nodeValue;
  }
  function textNodes(el) {
    var out = [], w = document.createTreeWalker(el, NodeFilter.SHOW_TEXT, null), n;
    while ((n = w.nextNode())) if (n.nodeValue.trim()) out.push(n);
    return out;
  }
  function blockKey(el) {
    var parts = [], slots = [], inline = 0, plain = false;
    for (var c = el.firstChild; c; c = c.nextSibling) {
      if (c.nodeType === 3) { var v = orig(c); if (v.trim()) plain = true; parts.push(v); slots.push({ text: c }); continue; }
      if (c.nodeType !== 1) continue;
      if (/^(svg|img|br)$/i.test(c.tagName)) continue;
      if (!INLINE.test(c.tagName)) return null;
      var tn = textNodes(c);
      if (tn.length !== 1) return null;
      var bad = false;
      c.querySelectorAll('*').forEach(function (e) { if (!SKIP_IN.test(e.tagName)) bad = true; });
      if (bad) return null;
      parts.push('[[' + norm(orig(tn[0])) + ']]');
      slots.push({ inline: tn[0] });
      inline++;
    }
    if (!plain || !inline) return null;
    return { key: norm(parts.join('')), slots: slots };
  }
  function put(n, value) { n.nodeValue = value; applied.set(n, value); }
  // app names stay English, also when they are split into parts ("Effect" + "Craft")
  function inName(el) {
    for (var i = 0; el && i < 3; i++, el = el.parentElement) if (/^[A-Z][A-Za-z]*Craft$/.test(norm(el.textContent))) return true;
    return false;
  }
  function translateTree(root) {
    if (lang !== 'de' || !DE) return;
    var done = new Set();
    var blocks = root.querySelectorAll ? root.querySelectorAll('p,h1,h2,h3,h4,h5,h6,li,a,button,figcaption,dt,dd,label,td,th,span,strong') : [];
    blocks.forEach(function (el) {
      if (el.closest('[data-ac-skip]') || inName(el)) return;
      var b = blockKey(el);
      if (!b || !DE[b.key]) return;
      var pieces = DE[b.key].split(/\[\[([\s\S]*?)\]\]/);
      var count = b.slots.filter(function (s) { return s.inline; }).length;
      if ((pieces.length - 1) / 2 !== count) return;
      var i = 0, first = true;
      b.slots.forEach(function (s) {
        if (s.inline) {
          var o = originals.get(s.inline) || s.inline.nodeValue;
          put(s.inline, o.match(/^\s*/)[0] + pieces[i + 1].trim() + o.match(/\s*$/)[0]);
          done.add(s.inline); i += 2; first = true;
        } else {
          var ov = originals.get(s.text) || s.text.nodeValue;
          if (first) {
            var piece = pieces[i];
            var lead = /^\s/.test(ov) || (i > 0 && /^\s/.test(piece)) ? ' ' : '';
            var trail = /\s$/.test(ov) || (i + 1 < pieces.length && /\s$/.test(piece)) ? ' ' : '';
            put(s.text, lead + piece.trim() + trail);
            first = false;
          } else put(s.text, '');
          done.add(s.text);
        }
      });
    });
    var w = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, null), n;
    while ((n = w.nextNode())) {
      if (done.has(n) || !n.parentElement || n.parentElement.closest('script,style,[data-ac-skip]') || inName(n.parentElement)) continue;
      var o = orig(n), k = norm(o);
      if (k && DE[k]) put(n, o.match(/^\s*/)[0] + DE[k] + o.match(/\s*$/)[0]);
    }
    if (root.querySelectorAll) {
      ['placeholder', 'title', 'aria-label', 'alt'].forEach(function (attr) {
        root.querySelectorAll('[' + attr + ']').forEach(function (el) {
          var o = el.getAttribute('data-en-' + attr);
          if (o === null) { o = el.getAttribute(attr); el.setAttribute('data-en-' + attr, o); }
          if (DE[norm(o)]) el.setAttribute(attr, DE[norm(o)]);
        });
      });
    }
  }
  function untranslate() {
    var w = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT, null), n;
    while ((n = w.nextNode())) if (applied.has(n) && applied.get(n) === n.nodeValue) { n.nodeValue = originals.get(n); applied.delete(n); }
    document.querySelectorAll('[data-en-title],[data-en-aria-label],[data-en-alt],[data-en-placeholder]').forEach(function (el) {
      ['placeholder', 'title', 'aria-label', 'alt'].forEach(function (attr) { var o = el.getAttribute('data-en-' + attr); if (o !== null) el.setAttribute(attr, o); });
    });
  }
  function setLang(l) {
    lang = l;
    try { localStorage.setItem(LKEY, l); } catch (e) {}
    html.lang = l;
    if (l === 'de') { loadDE().then(function () { translateTree(document.body); title(); }); } else { untranslate(); title(); }
    document.querySelectorAll('[data-ac-own="chooser"]').forEach(function (b) { b.removeAttribute('data-ac-state'); });
    if (ready) downloadPage();
    document.querySelectorAll('[data-ac-lang]').forEach(function (b) {
      var on = b.getAttribute('data-ac-lang') === l;
      b.style.opacity = on ? '1' : '.55';
      b.setAttribute('aria-pressed', on ? 'true' : 'false');
    });
  }
  var enTitle = null;
  function title() {
    if (enTitle === null) enTitle = document.title;
    document.title = lang === 'de' && DE && DE[norm(enTitle)] ? DE[norm(enTitle)] : enTitle;
  }
  var deLoading = null;
  function loadDE() {
    if (DE) return Promise.resolve(DE);
    if (!deLoading) deLoading = window.fetch(BASE + '/i18n/de.json').then(function (r) { return r.json(); }).then(function (d) { DE = d; return d; }).catch(function () { DE = {}; return DE; });
    return deLoading;
  }
  function langSwitch() {
    if (document.querySelector('[data-ac-own="lang"]')) return;
    var dl = document.querySelector('[data-ac-download]');
    var box = document.createElement('div');
    box.setAttribute('data-ac-own', 'lang');
    box.setAttribute('data-ac-skip', '');
    box.className = 'hidden md:flex items-center gap-1 border-l border-line px-3 hud-label';
    ['de', 'en'].forEach(function (l) {
      var b = document.createElement('button');
      b.type = 'button';
      b.textContent = l.toUpperCase();
      b.setAttribute('data-ac-lang', l);
      b.className = 'px-1.5 py-1 text-ink cursor-pointer';
      b.addEventListener('click', function () { setLang(l); });
      box.appendChild(b);
    });
    var holder = dl && dl.parentElement;
    if (holder) holder.insertBefore(box, holder.firstChild);
    else { box.className = 'hud-label'; box.style.cssText = 'position:fixed;right:1rem;bottom:1rem;z-index:99;display:flex;gap:.25rem;background:var(--bg);border:1px solid var(--line);padding:.25rem .5rem'; document.body.appendChild(box); }
  }

  // ---- putting it together ------------------------------------------------------------------
  var ready = false;
  function scan(root) {
    if (!root.querySelectorAll) return;
    if (root.tagName === 'A') fixLink(root);
    root.querySelectorAll('a[href]').forEach(fixLink);
    goneSections(root);
    hideByLabel(root.parentElement || root);
    if (ready) { texts(root); translateTree(root); }
  }
  function hydrated() {
    var n = document.querySelector('nav') || document.querySelector('main');
    if (!n) return false;
    for (var k in n) if (k.indexOf('__reactFiber') === 0 || k.indexOf('__reactProps') === 0) return true;
    return false;
  }
  var busy = false;
  var mo = new MutationObserver(function (list) {
    if (busy) return;
    busy = true;
    try {
      list.forEach(function (m) {
        if (m.type === 'attributes') { if (m.target.tagName === 'A') { fixLink(m.target); if (ready) downloadPage(); } return; }
        if (m.type === 'characterData') {
          var n = m.target;
          if (ready && lang === 'de' && DE && n.parentElement && !(applied.has(n) && applied.get(n) === n.nodeValue)) {
            originals.set(n, n.nodeValue);
            translateTree(n.parentElement.parentElement || n.parentElement);
          }
          if (ready && n.parentElement) texts(n.parentElement);
          return;
        }
        m.addedNodes.forEach(function (n) { if (n.nodeType === 1) scan(n); });
      });
      if (ready) { headerButton(); addOriginalLink(); langSwitch(); addCredits(); }
    } finally { busy = false; }
  });
  mo.observe(html, { childList: true, subtree: true, characterData: true, attributes: true, attributeFilter: ['href'] });

  if (lang === 'de') { html.classList.add('ac-wait'); loadDE(); }
  setTimeout(function () { html.classList.remove('ac-wait'); }, 3000);

  function start() {
    if (ready) return;
    ready = true;
    busy = true;
    try {
      scan(document.body);
      headerButton();
      texts(document.body);
      addOriginalLink();
      langSwitch();
      addCredits();
      html.classList.add('ac-ready');
    } finally { busy = false; }
    setLang(lang);
    if (lang === 'de') loadDE().then(function () { busy = true; try { translateTree(document.body); title(); } finally { busy = false; } html.classList.remove('ac-wait'); });
    else html.classList.remove('ac-wait');
  }
  var tries = 0;
  (function wait() {
    if (hydrated() || tries++ > 100) { setTimeout(start, 30); return; }
    setTimeout(wait, 40);
  })();

  // for the build: every text a visitor can see, sentences with inline parts marked [[...]]
  window.__acTexts = function () {
    var out = new Set();
    function vis(el) { return el && !el.closest('[data-ac-hide],[data-ac-skip],script,style,noscript'); }
    document.querySelectorAll('p,h1,h2,h3,h4,h5,h6,li,a,button,figcaption,dt,dd,label,td,th,span,strong').forEach(function (el) {
      if (!vis(el)) return;
      var b = blockKey(el);
      if (b) out.add(b.key);
    });
    var w = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT, null), n;
    while ((n = w.nextNode())) { if (vis(n.parentElement)) { var k = norm(orig(n)); if (k && /[A-Za-z]{2}/.test(k)) out.add(k); } }
    document.querySelectorAll('[alt],[title],[aria-label],[placeholder]').forEach(function (el) {
      if (!vis(el)) return;
      ['alt', 'title', 'aria-label', 'placeholder'].forEach(function (a) { var v = el.getAttribute('data-en-' + a) || el.getAttribute(a); if (v && /[A-Za-z]{2}/.test(v)) out.add(norm(v)); });
    });
    out.add(norm(enTitle || document.title));
    return Array.from(out);
  };
})();

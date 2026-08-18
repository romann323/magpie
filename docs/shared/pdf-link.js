/*
 * Injects a "Download PDF" button into the mdBook menu bar. The URL of
 * the PDF is derived from the site (../picorg-user-manual.pdf or
 * ../picorg-developer-guide.pdf) so this single script works for both
 * books without configuration.
 */
(function () {
  function whichPdf() {
    var p = (location.pathname || '').replace(/\\/g, '/');
    if (p.indexOf('/user-manual/') !== -1) {
      return { href: '../picorg-user-manual.pdf', label: 'PicOrg-User-Manual.pdf' };
    }
    if (p.indexOf('/developer/') !== -1) {
      return { href: '../picorg-developer-guide.pdf', label: 'PicOrg-Developer-Guide.pdf' };
    }
    // Local file:// preview: fall back to a sibling PDF.
    return { href: '../picorg-user-manual.pdf', label: 'Download PDF' };
  }

  function iconSvg() {
    return (
      '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">' +
      '<path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>' +
      '<polyline points="7 10 12 15 17 10"/>' +
      '<line x1="12" y1="15" x2="12" y2="3"/>' +
      '</svg>'
    );
  }

  function install() {
    var bar = document.getElementById('menu-bar');
    if (!bar) return;
    if (document.querySelector('.picorg-pdf-button')) return;

    var target = whichPdf();
    var a = document.createElement('a');
    a.className = 'picorg-pdf-button';
    a.href = target.href;
    a.download = target.label;
    a.title = 'Download this manual as PDF';
    a.setAttribute('aria-label', 'Download PDF');
    a.innerHTML = iconSvg() + '<span>Download PDF</span>';

    // Try to place it near the right side of the menu bar, next to the
    // search/print buttons. Fall back to appending to the bar.
    var right = bar.querySelector('.right-buttons');
    if (right) {
      right.appendChild(a);
    } else {
      bar.appendChild(a);
    }
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', install);
  } else {
    install();
  }
})();

/* ============================================================
   GemiClawdex Documentation — Navigation & Interactivity
   ============================================================ */

document.addEventListener('DOMContentLoaded', () => {
    const chapters = document.querySelectorAll('.chapter');
    const navItems = document.querySelectorAll('.nav-item');
    const searchInput = document.getElementById('searchInput');
    const progressBar = document.getElementById('progressBar');
    const progressText = document.getElementById('progressText');
    const mobileMenuBtn = document.getElementById('mobileMenuBtn');
    const sidebar = document.getElementById('sidebar');
    const backToTopBtn = document.getElementById('backToTop');
    const mainContent = document.getElementById('mainContent');

    const totalChapters = chapters.length;
    const visitedChapters = new Set(['home']);

    // --- Navigation ---
    function navigateTo(chapterId) {
        // Hide all chapters
        chapters.forEach(ch => {
            ch.classList.remove('active', 'fade-in');
        });

        // Show target chapter
        const target = document.getElementById(chapterId);
        if (target) {
            target.classList.add('active');
            // Trigger fade-in animation
            requestAnimationFrame(() => {
                target.classList.add('fade-in');
            });
        }

        // Update nav active state
        navItems.forEach(item => {
            item.classList.remove('active');
            if (item.dataset.chapter === chapterId) {
                item.classList.add('active');
                // Scroll nav item into view
                item.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
            }
        });

        // Track visited
        visitedChapters.add(chapterId);
        updateVisitedState();
        updateProgress();

        // Scroll main content to top
        mainContent.scrollTo({ top: 0, behavior: 'smooth' });
        window.scrollTo({ top: 0, behavior: 'smooth' });

        // Update URL hash without scrolling
        history.pushState(null, '', '#' + chapterId);

        // Close mobile menu
        sidebar.classList.remove('open');
        mobileMenuBtn.classList.remove('active');

        // Re-highlight code blocks
        if (window.Prism) {
            Prism.highlightAllUnder(target);
        }
    }

    // Make navigateTo globally accessible
    window.navigateTo = navigateTo;

    // Nav item click handlers
    navItems.forEach(item => {
        item.addEventListener('click', (e) => {
            e.preventDefault();
            const chapterId = item.dataset.chapter;
            if (chapterId) navigateTo(chapterId);
        });
    });

    // Handle initial hash
    const initialHash = window.location.hash.slice(1);
    if (initialHash && document.getElementById(initialHash)) {
        navigateTo(initialHash);
    }

    // Handle browser back/forward
    window.addEventListener('popstate', () => {
        const hash = window.location.hash.slice(1) || 'home';
        if (document.getElementById(hash)) {
            navigateTo(hash);
        }
    });

    // --- Visited State ---
    function updateVisitedState() {
        navItems.forEach(item => {
            if (visitedChapters.has(item.dataset.chapter) && !item.classList.contains('active')) {
                item.classList.add('visited');
            }
        });
    }

    // --- Progress ---
    function updateProgress() {
        const progress = (visitedChapters.size / totalChapters) * 100;
        progressBar.style.width = progress + '%';
        progressText.textContent = `${visitedChapters.size} / ${totalChapters} 章`;
    }
    updateProgress();

    // --- Search ---
    searchInput.addEventListener('input', (e) => {
        const query = e.target.value.toLowerCase().trim();
        navItems.forEach(item => {
            const text = item.textContent.toLowerCase();
            if (query === '' || text.includes(query)) {
                item.classList.remove('search-hidden');
            } else {
                item.classList.add('search-hidden');
            }
        });
    });

    // --- Mobile Menu ---
    mobileMenuBtn.addEventListener('click', () => {
        sidebar.classList.toggle('open');
        mobileMenuBtn.classList.toggle('active');
    });

    // Close sidebar on outside click (mobile)
    mainContent.addEventListener('click', () => {
        if (window.innerWidth <= 768) {
            sidebar.classList.remove('open');
            mobileMenuBtn.classList.remove('active');
        }
    });

    // --- Back to Top ---
    window.addEventListener('scroll', () => {
        if (window.scrollY > 300) {
            backToTopBtn.classList.add('visible');
        } else {
            backToTopBtn.classList.remove('visible');
        }
    });

    backToTopBtn.addEventListener('click', () => {
        window.scrollTo({ top: 0, behavior: 'smooth' });
    });

    // --- Keyboard Navigation ---
    document.addEventListener('keydown', (e) => {
        // Ctrl+K or Cmd+K to focus search
        if ((e.ctrlKey || e.metaKey) && e.key === 'k') {
            e.preventDefault();
            searchInput.focus();
        }

        // Escape to clear search
        if (e.key === 'Escape') {
            searchInput.value = '';
            searchInput.dispatchEvent(new Event('input'));
            searchInput.blur();
        }

        // Arrow keys for chapter navigation (when not in input)
        if (document.activeElement === searchInput) return;

        const chapterIds = Array.from(navItems).map(item => item.dataset.chapter);
        const currentChapter = document.querySelector('.chapter.active');
        const currentId = currentChapter ? currentChapter.id : 'home';
        const currentIndex = chapterIds.indexOf(currentId);

        if (e.key === 'ArrowRight' || e.key === 'ArrowDown') {
            if (currentIndex < chapterIds.length - 1) {
                e.preventDefault();
                navigateTo(chapterIds[currentIndex + 1]);
            }
        }
        if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') {
            if (currentIndex > 0) {
                e.preventDefault();
                navigateTo(chapterIds[currentIndex - 1]);
            }
        }
    });

    // --- Handle clicks on in-content navigation links ---
    document.querySelectorAll('.chapter-nav .btn, .hero-actions .btn').forEach(link => {
        link.addEventListener('click', (e) => {
            const href = link.getAttribute('href');
            if (href && href.startsWith('#')) {
                e.preventDefault();
                navigateTo(href.slice(1));
            }
        });
    });
});

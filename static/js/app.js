// LoopKaraoke Studio - Frontend Application State & Logic

const state = {
    currentStep: 1,
    video: null, // { path, filename, duration, duration_str, width, height, aspect_ratio, web_url }
    aspectRatio: "16:9",
    fitMode: "crop",
    muteVideo: true,
    tracks: [], // [ { id, path, filename, title, artist, duration, duration_str, web_url, lyrics: [] } ]
    activeTrackId: null,
    shufflePlaylist: true, // Default ON: acak otomatis urutan lagu
    imageBeatPulse: true, // Default ON: efek zoom denyut mengikuti irama lagu
    visualEffect: "none", // Atmosfer & Filter: none, rain, snow, fire, bokeh, light_leak, color_grading, vhs, glitch
    styleConfig: {
        theme: "golden",
        normal_color: "#FFFFFF",
        singing_color: "#FFD700",
        outline_color: "#000000",
        shadow_color: "#1A1A1A",
        font_name: "Arial",
        font_size: 48,
        alignment: 2,
        margin_v: 70,
        wipe_mode: "smooth",
        show_intro: true
    },
    renderConfig: {
        resolution: "1080p",
        fps: 30
    },
    audioPlayer: new Audio(),
    isPlayingKaraoke: false,
    renderInterval: null,
    thumb: {
        backgroundImg: null,
        videoTimestamp: 0.0,
        title: "Powerful Worship",
        subtitle: "Hillsong Worship",
        tracks: [],
        columnMode: "auto",
        numberStyle: "mirrored",
        titleSize: 130,
        trackSize: 36,
        titleFont: "Impact",
        subFont: "Great Vibes",
        titleColor: "#FFFFFF",
        subtitleColor: "#FFD700",
        vignette: 55,
        isInitialized: false
    }
};

// ================= INITIALIZATION =================
document.addEventListener("DOMContentLoaded", () => {
    initNavigation();
    initVideoUpload();
    initPlaylist();
    initKaraokeStudio();
    initRenderExport();
    initQuickDemo();
    initStorageManager();
    initThumbnailGenerator();
});

// ================= 1. NAVIGATION =================
function initNavigation() {
    document.querySelectorAll(".step-btn").forEach(btn => {
        btn.addEventListener("click", () => {
            const step = parseInt(btn.dataset.step);
            goToStep(step);
        });
    });
}

function goToStep(stepNumber) {
    if (stepNumber === 2 && !state.video) {
        alert("Silakan pilih atau unggah video background terlebih dahulu.");
        return;
    }
    if (stepNumber === 3 && state.tracks.length === 0) {
        alert("Silakan tambahkan minimal 1 lagu dalam daftar putar.");
        return;
    }

    state.currentStep = stepNumber;

    // Update step buttons
    document.querySelectorAll(".step-btn").forEach(btn => {
        const s = parseInt(btn.dataset.step);
        btn.classList.toggle("active", s === stepNumber);
        btn.classList.toggle("completed", s < stepNumber);
    });

    // Update panels
    document.querySelectorAll(".step-panel").forEach((panel, idx) => {
        panel.classList.toggle("active", idx + 1 === stepNumber);
    });

    // Step-specific updates
    if (stepNumber === 3) {
        updateTrackSelectDropdown();
    }
    if (stepNumber === 4) {
        updateRenderSummary();
    }
    if (stepNumber === 5) {
        onEnterThumbnailStep();
    }

    lucide.createIcons();
    window.scrollTo({ top: 0, behavior: 'smooth' });
}

// ================= 2. VIDEO & IMAGE BACKGROUND HANDLER =================
let activeBgTab = "video"; // "video" or "image"
let lastImageFile = null;

function initVideoUpload() {
    const videoDropzone = document.getElementById("video-dropzone");
    const videoFileInput = document.getElementById("video-file-input");
    const imageDropzone = document.getElementById("image-dropzone");
    const imageFileInput = document.getElementById("image-file-input");
    const changeBtn = document.getElementById("btn-change-video");

    const tabVideo = document.getElementById("tab-bg-video");
    const tabImage = document.getElementById("tab-bg-image");
    const imageDropzoneSection = document.getElementById("image-dropzone-section");

    // Tab switcher (Video vs Image)
    if (tabVideo && tabImage) {
        tabVideo.addEventListener("click", () => {
            activeBgTab = "video";
            tabVideo.className = "btn btn-sm btn-primary flex-1";
            tabImage.className = "btn btn-sm btn-ghost flex-1";
            videoDropzone.classList.remove("hidden");
            if (imageDropzoneSection) imageDropzoneSection.classList.add("hidden");
        });

        tabImage.addEventListener("click", () => {
            activeBgTab = "image";
            tabImage.className = "btn btn-sm btn-primary flex-1";
            tabVideo.className = "btn btn-sm btn-ghost flex-1";
            videoDropzone.classList.add("hidden");
            if (imageDropzoneSection) imageDropzoneSection.classList.remove("hidden");
        });
    }

    // Video Drag & Drop
    ["dragenter", "dragover"].forEach(eventName => {
        videoDropzone.addEventListener(eventName, (e) => {
            e.preventDefault();
            videoDropzone.classList.add("dragover");
        });
    });
    ["dragleave", "drop"].forEach(eventName => {
        videoDropzone.addEventListener(eventName, (e) => {
            e.preventDefault();
            videoDropzone.classList.remove("dragover");
        });
    });

    videoDropzone.addEventListener("drop", (e) => {
        const file = e.dataTransfer.files[0];
        if (!file) return;
        if (isImageFile(file)) {
            uploadImageToVideo(file);
        } else {
            uploadVideoFile(file);
        }
    });

    videoFileInput.addEventListener("change", (e) => {
        const file = e.target.files[0];
        if (!file) return;
        if (isImageFile(file)) {
            uploadImageToVideo(file);
        } else {
            uploadVideoFile(file);
        }
    });

    // Image Drag & Drop
    if (imageDropzone) {
        ["dragenter", "dragover"].forEach(evt => {
            imageDropzone.addEventListener(evt, (e) => {
                e.preventDefault();
                imageDropzone.classList.add("dragover");
            });
        });
        ["dragleave", "drop"].forEach(evt => {
            imageDropzone.addEventListener(evt, (e) => {
                e.preventDefault();
                imageDropzone.classList.remove("dragover");
            });
        });

        imageDropzone.addEventListener("drop", (e) => {
            const file = e.dataTransfer.files[0];
            if (file) uploadImageToVideo(file);
        });
    }

    if (imageFileInput) {
        imageFileInput.addEventListener("change", (e) => {
            const file = e.target.files[0];
            if (file) uploadImageToVideo(file);
        });
    }

    // Visual effect, duration, beat pulse, or atmosphere change listeners
    const effectSelect = document.getElementById("img-effect-select");
    const durSelect = document.getElementById("img-duration-select");
    const beatPulseToggle = document.getElementById("img-beat-pulse-toggle");
    const visualEffectSelect = document.getElementById("img-visual-effect-select");

    [effectSelect, durSelect].forEach(el => {
        if (el) {
            el.addEventListener("change", () => {
                if (lastImageFile) {
                    uploadImageToVideo(lastImageFile);
                }
            });
        }
    });

    if (beatPulseToggle) {
        beatPulseToggle.checked = state.imageBeatPulse;
        beatPulseToggle.addEventListener("change", (e) => {
            state.imageBeatPulse = e.target.checked;
            if (lastImageFile) {
                uploadImageToVideo(lastImageFile);
            }
        });
    }

    if (visualEffectSelect) {
        visualEffectSelect.addEventListener("change", (e) => {
            state.visualEffect = e.target.value;
            const renderVe = document.getElementById("render-visual-effect-select");
            if (renderVe) renderVe.value = e.target.value;
            if (lastImageFile) {
                uploadImageToVideo(lastImageFile);
            }
        });
    }

    if (changeBtn) {
        changeBtn.addEventListener("click", () => {
            const previewWrapper = document.getElementById("video-preview-wrapper");
            previewWrapper.classList.add("hidden");
            if (activeBgTab === "image") {
                if (imageDropzoneSection) imageDropzoneSection.classList.remove("hidden");
                videoDropzone.classList.add("hidden");
            } else {
                videoDropzone.classList.remove("hidden");
                if (imageDropzoneSection) imageDropzoneSection.classList.add("hidden");
            }
        });
    }

    // Radio aspect ratio
    document.querySelectorAll('input[name="aspect-ratio"]').forEach(r => {
        r.addEventListener("change", (e) => {
            state.aspectRatio = e.target.value;
            document.querySelectorAll('.radio-card').forEach(c => c.classList.remove('active'));
            e.target.closest('.radio-card').classList.add('active');
            if (lastImageFile) {
                uploadImageToVideo(lastImageFile);
            }
        });
    });

    // Fit mode
    const fitSelect = document.getElementById("fit-mode-select");
    if (fitSelect) {
        fitSelect.addEventListener("change", (e) => {
            state.fitMode = e.target.value;
        });
    }

    // Mute toggle
    const muteToggle = document.getElementById("mute-video-toggle");
    if (muteToggle) {
        muteToggle.addEventListener("change", (e) => {
            state.muteVideo = e.target.checked;
        });
    }
}

function isImageFile(file) {
    if (!file) return false;
    if (file.type && file.type.startsWith("image/")) return true;
    return /\.(jpe?g|png|webp|jfif)$/i.test(file.name);
}

function formatEffectName(effect) {
    switch (effect) {
        case "zoom_in": return "Zoom In";
        case "zoom_out": return "Zoom Out";
        case "pan_left": return "Pan Kiri";
        case "pan_right": return "Pan Kanan";
        case "static": return "Statis";
        default: return effect;
    }
}

async function uploadVideoFile(file) {
    const formData = new FormData();
    formData.append("file", file);

    const badge = document.getElementById("video-badge");
    badge.textContent = "Mengunggah video...";
    badge.className = "badge badge-primary";

    try {
        const res = await fetch("/api/upload/video", {
            method: "POST",
            body: formData
        });
        const data = await res.json();
        if (!res.ok) throw new Error(data.detail || "Gagal mengunggah video.");

        state.video = data;
        displayVideoPreview(data);
        calculateLoopStats();

        badge.textContent = "Video Siap";
        badge.className = "badge badge-success";
    } catch (err) {
        alert(err.message);
        badge.textContent = "Gagal";
        badge.className = "badge badge-dim";
    }
}

async function uploadImageToVideo(file) {
    lastImageFile = file;
    const badge = document.getElementById("video-badge");
    badge.textContent = "Mengonversi gambar ke video...";
    badge.className = "badge badge-primary";

    const effect = document.getElementById("img-effect-select") ? document.getElementById("img-effect-select").value : "zoom_in";
    const duration = document.getElementById("img-duration-select") ? parseFloat(document.getElementById("img-duration-select").value) : 10.0;
    const beatPulse = document.getElementById("img-beat-pulse-toggle") ? document.getElementById("img-beat-pulse-toggle").checked : true;
    const visualEffect = document.getElementById("img-visual-effect-select") ? document.getElementById("img-visual-effect-select").value : (state.visualEffect || "none");
    state.imageBeatPulse = beatPulse;
    state.visualEffect = visualEffect;

    const formData = new FormData();
    formData.append("file", file);
    formData.append("duration", duration);
    formData.append("fps", 30);
    formData.append("aspect_ratio", state.aspectRatio || "16:9");
    formData.append("resolution", (state.renderConfig && state.renderConfig.resolution) || "1080p");
    formData.append("effect", effect);
    formData.append("beat_pulse", beatPulse);
    formData.append("visual_effect", visualEffect);

    try {
        const res = await fetch("/api/upload/image-to-video", {
            method: "POST",
            body: formData
        });
        const data = await res.json();
        if (!res.ok) throw new Error(data.detail || "Gagal mengonversi gambar ke video.");

        state.video = data;
        displayVideoPreview(data);
        calculateLoopStats();

        badge.textContent = "Gambar Siap (Video)";
        badge.className = "badge badge-success";
    } catch (err) {
        alert(err.message);
        badge.textContent = "Gagal";
        badge.className = "badge badge-dim";
    }
}

function displayVideoPreview(video) {
    const videoDropzone = document.getElementById("video-dropzone");
    const imageDropzoneSection = document.getElementById("image-dropzone-section");
    const previewWrapper = document.getElementById("video-preview-wrapper");
    const videoPlayer = document.getElementById("video-player");

    if (videoDropzone) videoDropzone.classList.add("hidden");
    if (imageDropzoneSection) imageDropzoneSection.classList.add("hidden");
    previewWrapper.classList.remove("hidden");

    videoPlayer.src = video.web_url;
    videoPlayer.load();

    const iconType = video.is_from_image ? "image" : "file-video";
    const displayName = video.source_image_name || video.filename;
    const beatSuffix = (video.is_from_image && video.beat_pulse) ? " • Beat Sync" : "";
    const veLabels = {
        rain: " • 🌧️ Hujan",
        snow: " • ❄️ Salju",
        fire: " • 🔥 Bara Api",
        bokeh: " • ✨ Bokeh",
        light_leak: " • 🌅 Light Leak",
        color_grading: " • 🎨 Teal & Orange",
        vhs: " • 📼 VHS",
        glitch: " • ⚡ Glitch"
    };
    const veSuffix = (video.visual_effect && veLabels[video.visual_effect]) ? veLabels[video.visual_effect] : "";
    const effectLabel = video.effect ? ` (${formatEffectName(video.effect)}${beatSuffix}${veSuffix})` : "";

    document.getElementById("video-meta-name").innerHTML = `<i data-lucide="${iconType}"></i> ${escapeHtml(displayName)}${effectLabel}`;
    document.getElementById("video-meta-duration").innerHTML = `<i data-lucide="clock"></i> ${video.duration_str}`;
    document.getElementById("video-meta-res").innerHTML = `<i data-lucide="maximize"></i> ${video.width}x${video.height} (${video.aspect_ratio})`;

    lucide.createIcons();
}

// ================= 3. PLAYLIST HANDLER =================
function initPlaylist() {
    const audioDropzone = document.getElementById("audio-dropzone");
    const audioInput = document.getElementById("audio-file-input");
    const shuffleToggle = document.getElementById("shuffle-playlist-toggle");
    const btnShuffleNow = document.getElementById("btn-shuffle-now");

    if (shuffleToggle) {
        shuffleToggle.checked = state.shufflePlaylist;
        shuffleToggle.addEventListener("change", (e) => {
            state.shufflePlaylist = e.target.checked;
            if (state.shufflePlaylist && state.tracks.length > 1) {
                shufflePlaylist(false);
            }
        });
    }

    if (btnShuffleNow) {
        btnShuffleNow.addEventListener("click", () => {
            shufflePlaylist(true);
        });
    }

    ["dragenter", "dragover"].forEach(evt => {
        audioDropzone.addEventListener(evt, (e) => {
            e.preventDefault();
            audioDropzone.classList.add("dragover");
        });
    });
    ["dragleave", "drop"].forEach(evt => {
        audioDropzone.addEventListener(evt, (e) => {
            e.preventDefault();
            audioDropzone.classList.remove("dragover");
        });
    });

    audioDropzone.addEventListener("drop", (e) => {
        const files = Array.from(e.dataTransfer.files);
        if (files.length > 0) uploadAudioFiles(files);
    });

    audioInput.addEventListener("change", (e) => {
        const files = Array.from(e.target.files);
        if (files.length > 0) uploadAudioFiles(files);
    });
}

function shufflePlaylist(showAlert = false) {
    if (state.tracks.length <= 1) {
        if (showAlert) alert("Tambahkan minimal 2 lagu untuk mengacak urutan daftar putar.");
        return;
    }

    // Fisher-Yates shuffle algorithm
    for (let i = state.tracks.length - 1; i > 0; i--) {
        const j = Math.floor(Math.random() * (i + 1));
        [state.tracks[i], state.tracks[j]] = [state.tracks[j], state.tracks[i]];
    }

    renderPlaylist();
    calculateLoopStats();
    updateTrackSelectDropdown();
    updateYouTubeTimestampsUI();
    if (state.thumb && state.thumb.isInitialized) {
        syncTracksToThumbnail(true);
    }
}

async function uploadAudioFiles(files) {
    const formData = new FormData();
    files.forEach(f => formData.append("files", f));

    try {
        const res = await fetch("/api/upload/audios", {
            method: "POST",
            body: formData
        });
        const data = await res.json();
        if (!res.ok) throw new Error(data.detail || "Gagal mengunggah file musik.");

        data.forEach(t => {
            if (t.title) t.title = cleanProperTitle(t.title);
            state.tracks.push(t);
        });

        if (state.shufflePlaylist && state.tracks.length > 1) {
            shufflePlaylist(false);
        } else {
            renderPlaylist();
            calculateLoopStats();
            updateTrackSelectDropdown();
            updateYouTubeTimestampsUI();
        }
    } catch (err) {
        alert(err.message);
    }
}

function renderPlaylist() {
    const container = document.getElementById("playlist-container");
    const dropzone = document.getElementById("audio-dropzone");

    if (state.tracks.length === 0) {
        dropzone.classList.remove("hidden");
        container.innerHTML = "";
        return;
    }

    dropzone.classList.add("hidden");
    container.innerHTML = state.tracks.map((track, idx) => `
        <div class="playlist-item" data-id="${track.id}">
            <div class="playlist-item-left">
                <span class="track-index">${idx + 1}</span>
                <button class="btn-icon-sm" onclick="playPreviewTrack('${track.id}')" title="Putar lagu">
                    <i data-lucide="play"></i>
                </button>
                <div class="track-meta">
                    <span class="track-title">${escapeHtml(cleanProperTitle(track.title || track.filename))}</span>
                    <span class="track-sub">${escapeHtml(track.artist ? track.artist + ' • ' : '')}${track.duration_str}</span>
                </div>
            </div>
            <div class="playlist-item-actions">
                <button class="btn-icon-sm" onclick="moveTrack(${idx}, -1)" title="Pindah ke atas" ${idx === 0 ? 'disabled' : ''}>
                    <i data-lucide="chevron-up"></i>
                </button>
                <button class="btn-icon-sm" onclick="moveTrack(${idx}, 1)" title="Pindah ke bawah" ${idx === state.tracks.length - 1 ? 'disabled' : ''}>
                    <i data-lucide="chevron-down"></i>
                </button>
                <button class="btn-icon-sm text-danger" onclick="removeTrack('${track.id}')" title="Hapus lagu">
                    <i data-lucide="trash-2"></i>
                </button>
            </div>
        </div>
    `).join("");

    lucide.createIcons();
}

function moveTrack(index, direction) {
    const targetIndex = index + direction;
    if (targetIndex < 0 || targetIndex >= state.tracks.length) return;
    const item = state.tracks.splice(index, 1)[0];
    state.tracks.splice(targetIndex, 0, item);
    renderPlaylist();
    updateYouTubeTimestampsUI();
}

function removeTrack(trackId) {
    state.tracks = state.tracks.filter(t => t.id !== trackId);
    if (state.activeTrackId === trackId) {
        state.activeTrackId = state.tracks.length > 0 ? state.tracks[0].id : null;
    }
    renderPlaylist();
    calculateLoopStats();
    updateYouTubeTimestampsUI();
}

function playPreviewTrack(trackId) {
    const track = state.tracks.find(t => t.id === trackId);
    if (!track) return;

    if (state.audioPlayer.src === window.location.origin + track.web_url && !state.audioPlayer.paused) {
        state.audioPlayer.pause();
    } else {
        state.audioPlayer.src = track.web_url;
        state.audioPlayer.play();
    }
}

function calculateLoopStats() {
    const countSpan = document.getElementById("stat-song-count");
    const totalSpan = document.getElementById("stat-total-duration");
    const videoSpan = document.getElementById("stat-video-duration");
    const loopTitle = document.getElementById("loop-calc-title");
    const loopDesc = document.getElementById("loop-calc-desc");

    countSpan.textContent = `${state.tracks.length} Lagu`;

    const totalAudioSeconds = state.tracks.reduce((acc, t) => acc + (t.duration || 0), 0);
    totalSpan.textContent = formatSec(totalAudioSeconds);

    const videoSeconds = state.video ? (state.video.duration || 0) : 0;
    videoSpan.textContent = state.video ? state.video.duration_str : "00:00";

    if (state.video && state.tracks.length > 0 && videoSeconds > 0) {
        const loopCount = Math.ceil(totalAudioSeconds / videoSeconds);
        loopTitle.textContent = `Video Akan Meloop ${loopCount} Kali`;
        loopDesc.textContent = `Video background (${state.video.duration_str}) akan berputar terus menerus sampai seluruh ${state.tracks.length} lagu (${formatSec(totalAudioSeconds)}) selesai dimainkan.`;
    } else {
        loopTitle.textContent = "Estimasi Looping Video";
        loopDesc.textContent = "Tambahkan video dan lagu untuk melihat berapa kali video akan meloop.";
    }
}

// ================= 4. KARAOKE STUDIO =================
function initKaraokeStudio() {
    const trackSelect = document.getElementById("lyric-track-select");
    const btnTranscribe = document.getElementById("btn-transcribe-ai");
    const btnImportLrc = document.getElementById("btn-import-lrc");
    const lrcFileInput = document.getElementById("lrc-file-input");
    const btnPasteText = document.getElementById("btn-paste-text");
    const playToggleBtn = document.getElementById("karaoke-play-toggle");
    const seekBar = document.getElementById("karaoke-seek-bar");

    trackSelect.addEventListener("change", (e) => {
        state.activeTrackId = e.target.value;
        loadActiveTrackLyrics();
    });

    btnTranscribe.addEventListener("click", runAiTranscription);

    const btnTranscribeAll = document.getElementById("btn-transcribe-all-ai");
    if (btnTranscribeAll) {
        btnTranscribeAll.addEventListener("click", runAiTranscriptionAll);
    }

    btnImportLrc.addEventListener("click", () => lrcFileInput.click());
    lrcFileInput.addEventListener("change", handleLrcFileUpload);

    btnPasteText.addEventListener("click", () => {
        document.getElementById("paste-modal").classList.remove("hidden");
    });
    document.getElementById("btn-save-pasted-lrc").addEventListener("click", handlePastedLrc);

    // Audio & Karaoke sync
    playToggleBtn.addEventListener("click", toggleKaraokePlay);

    state.audioPlayer.addEventListener("timeupdate", () => {
        if (!state.activeTrackId) return;
        const cur = state.audioPlayer.currentTime;
        const dur = state.audioPlayer.duration || 1;

        document.getElementById("karaoke-time-current").textContent = formatSec(cur);
        seekBar.value = (cur / dur) * 100;

        renderLiveKaraokeDisplay(cur);
    });

    state.audioPlayer.addEventListener("loadedmetadata", () => {
        document.getElementById("karaoke-time-total").textContent = formatSec(state.audioPlayer.duration || 0);
    });

    state.audioPlayer.addEventListener("ended", () => {
        state.isPlayingKaraoke = false;
        playToggleBtn.innerHTML = '<i data-lucide="play"></i>';
        lucide.createIcons();
    });

    seekBar.addEventListener("input", (e) => {
        if (state.audioPlayer.duration) {
            state.audioPlayer.currentTime = (e.target.value / 100) * state.audioPlayer.duration;
        }
    });

    // Theme selector
    const themeSelect = document.getElementById("preset-theme-select");
    themeSelect.addEventListener("change", (e) => {
        applyPresetTheme(e.target.value);
    });

    // Custom colors
    document.getElementById("color-normal").addEventListener("input", (e) => state.styleConfig.normal_color = e.target.value);
    document.getElementById("color-singing").addEventListener("input", (e) => {
        state.styleConfig.singing_color = e.target.value;
        document.documentElement.style.setProperty('--accent-gold', e.target.value);
    });
    document.getElementById("color-outline").addEventListener("input", (e) => state.styleConfig.outline_color = e.target.value);

    // Font size
    const fontSlider = document.getElementById("font-size-slider");
    fontSlider.addEventListener("input", (e) => {
        state.styleConfig.font_size = parseInt(e.target.value);
        document.getElementById("font-size-val").textContent = e.target.value;
    });

    // Alignment
    document.querySelectorAll(".seg-btn").forEach(btn => {
        btn.addEventListener("click", (e) => {
            document.querySelectorAll(".seg-btn").forEach(b => b.classList.remove("active"));
            btn.classList.add("active");
            state.styleConfig.alignment = parseInt(btn.dataset.align);
        });
    });

    // Wipe mode
    document.getElementById("wipe-mode-select").addEventListener("change", (e) => {
        state.styleConfig.wipe_mode = e.target.value;
    });

    // Show intro
    document.getElementById("show-intro-toggle").addEventListener("change", (e) => {
        state.styleConfig.show_intro = e.target.checked;
    });

    // Add line / clear
    document.getElementById("btn-add-line").addEventListener("click", addNewLyricLine);
    document.getElementById("btn-clear-lyrics").addEventListener("click", clearActiveTrackLyrics);
}

function updateTrackSelectDropdown() {
    const select = document.getElementById("lyric-track-select");
    select.innerHTML = state.tracks.map(t => `
        <option value="${t.id}" ${t.id === state.activeTrackId ? 'selected' : ''}>
            ${escapeHtml(t.title)} (${t.duration_str})
        </option>
    `).join("");

    if (!state.activeTrackId && state.tracks.length > 0) {
        state.activeTrackId = state.tracks[0].id;
        select.value = state.activeTrackId;
    }
    loadActiveTrackLyrics();
}

function loadActiveTrackLyrics() {
    const track = state.tracks.find(t => t.id === state.activeTrackId);
    if (!track) return;

    document.getElementById("active-track-name").textContent = `Lirik: ${track.title}`;
    state.audioPlayer.src = track.web_url;
    state.audioPlayer.load();

    renderLyricsTable(track.lyrics || []);
}

async function runAiTranscription() {
    const track = state.tracks.find(t => t.id === state.activeTrackId);
    if (!track) {
        alert("Pilih lagu terlebih dahulu.");
        return;
    }

    const btn = document.getElementById("btn-transcribe-ai");
    const originalText = btn.innerHTML;
    btn.disabled = true;
    btn.innerHTML = `<i data-lucide="loader-2" class="spin"></i> AI Sedang Mendeteksi Lirik...`;
    lucide.createIcons();

    const modelSize = document.getElementById("whisper-model-select").value;
    const language = document.getElementById("whisper-lang-select").value;
    const device = document.getElementById("whisper-device-select") ? document.getElementById("whisper-device-select").value : "cuda";

    try {
        const res = await fetch("/api/transcribe", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({
                audio_path: track.path,
                model_size: modelSize,
                language: language,
                device: device
            })
        });
        const data = await res.json();
        if (!res.ok) throw new Error(data.detail || "Gagal mentranskripsi.");

        track.lyrics = data.lyrics;
        renderLyricsTable(track.lyrics);
        updateRenderSummary();
        alert(`Berhasil mendeteksi ${track.lyrics.length} baris lirik karaoke untuk "${track.title}"!`);
    } catch (err) {
        alert(err.message);
    } finally {
        btn.disabled = false;
        btn.innerHTML = originalText;
        lucide.createIcons();
    }
}

async function runAiTranscriptionAll() {
    if (!state.tracks || state.tracks.length === 0) {
        alert("Daftar lagu masih kosong. Unggah minimal 1 lagu di Langkah 2 terlebih dahulu.");
        return;
    }

    const btnSingle = document.getElementById("btn-transcribe-ai");
    const btnAll = document.getElementById("btn-transcribe-all-ai");
    const origSingleHtml = btnSingle ? btnSingle.innerHTML : "";
    const origAllHtml = btnAll ? btnAll.innerHTML : "";

    const progressBox = document.getElementById("batch-transcribe-progress");
    const statusText = document.getElementById("batch-transcribe-status");
    const percentText = document.getElementById("batch-transcribe-percent");
    const progressBar = document.getElementById("batch-transcribe-bar");

    if (btnSingle) btnSingle.disabled = true;
    if (btnAll) {
        btnAll.disabled = true;
        btnAll.innerHTML = `<i data-lucide="loader-2" class="spin"></i> AI Mendeteksi Semua Lagu...`;
        lucide.createIcons();
    }
    if (progressBox) progressBox.classList.remove("hidden");

    const modelSize = document.getElementById("whisper-model-select").value;
    const language = document.getElementById("whisper-lang-select").value;
    const device = document.getElementById("whisper-device-select") ? document.getElementById("whisper-device-select").value : "cuda";

    const total = state.tracks.length;
    let successCount = 0;
    let failCount = 0;

    for (let i = 0; i < total; i++) {
        const track = state.tracks[i];
        const cleanName = cleanProperTitle(track.title || track.filename);
        const percentVal = Math.round((i / total) * 100);

        if (statusText) statusText.textContent = `Memproses (${i + 1}/${total}): "${cleanName}"...`;
        if (percentText) percentText.textContent = `${percentVal}%`;
        if (progressBar) progressBar.style.width = `${percentVal}%`;

        // Switch active track view so user sees progress in real time
        state.activeTrackId = track.id;
        const select = document.getElementById("lyric-track-select");
        if (select) select.value = track.id;
        loadActiveTrackLyrics();

        try {
            const res = await fetch("/api/transcribe", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({
                    audio_path: track.path,
                    model_size: modelSize,
                    language: language,
                    device: device
                })
            });
            const data = await res.json();
            if (!res.ok) throw new Error(data.detail || "Gagal mentranskripsi.");

            track.lyrics = data.lyrics;
            if (state.activeTrackId === track.id) {
                renderLyricsTable(track.lyrics);
            }
            successCount++;
        } catch (err) {
            console.error(`Error transcribing ${track.title}:`, err);
            failCount++;
        }
    }

    // Finished all tracks
    if (percentText) percentText.textContent = "100%";
    if (progressBar) progressBar.style.width = "100%";
    if (statusText) statusText.textContent = `Selesai! ${successCount} dari ${total} lagu berhasil dibuat liriknya.`;

    // Refresh render summary & track selector
    updateTrackSelectDropdown();
    updateRenderSummary();

    if (btnSingle) btnSingle.disabled = false;
    if (btnAll) {
        btnAll.disabled = false;
        btnAll.innerHTML = origAllHtml;
        lucide.createIcons();
    }

    setTimeout(() => {
        if (progressBox) progressBox.classList.add("hidden");
    }, 4000);

    alert(`Proses Auto Lirik Selesai!\n${successCount} dari ${total} lagu berhasil dibuatkan lirik karaoke otomatis.`);
}

async function handleLrcFileUpload(e) {
    const file = e.target.files[0];
    if (!file) return;

    const text = await file.text();
    applyLrcText(text);
}

function handlePastedLrc() {
    const text = document.getElementById("paste-textarea").value;
    if (text.trim()) {
        applyLrcText(text);
        closePasteModal();
    }
}

async function applyLrcText(lrcContent) {
    const track = state.tracks.find(t => t.id === state.activeTrackId);
    if (!track) return;

    try {
        const res = await fetch("/api/parse-lrc", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ lrc_content: lrcContent })
        });
        const data = await res.json();
        if (!res.ok) throw new Error(data.detail || "Gagal memproses lirik.");

        track.lyrics = data.lyrics;
        renderLyricsTable(track.lyrics);
    } catch (err) {
        alert(err.message);
    }
}

function renderLyricsTable(lyrics) {
    const container = document.getElementById("lyrics-table-container");
    if (!lyrics || lyrics.length === 0) {
        container.innerHTML = `
            <div style="text-align: center; padding: 40px; color: var(--text-muted); font-size: 13px;">
                Belum ada lirik untuk lagu ini.<br>Klik tombol <strong>"Buat Auto Lirik AI Sekarang"</strong> di sebelah kiri untuk membuat lirik karaoke otomatis.
            </div>
        `;
        return;
    }

    container.innerHTML = lyrics.map((line, idx) => `
        <div class="lyric-line-item" onclick="seekAudioTo(${line.start})" style="cursor: pointer;">
            <div style="display: flex; align-items: center; flex: 1;">
                <span class="lyric-time-badge">${formatSec(line.start)}</span>
                <span class="lyric-line-text">${escapeHtml(line.text)}</span>
            </div>
            <button class="btn-icon-sm text-danger" onclick="event.stopPropagation(); removeLyricLine(${idx});" title="Hapus baris">
                <i data-lucide="x"></i>
            </button>
        </div>
    `).join("");

    lucide.createIcons();
}

function seekAudioTo(seconds) {
    state.audioPlayer.currentTime = seconds;
    if (state.audioPlayer.paused) {
        toggleKaraokePlay();
    }
}

function removeLyricLine(index) {
    const track = state.tracks.find(t => t.id === state.activeTrackId);
    if (track && track.lyrics) {
        track.lyrics.splice(index, 1);
        renderLyricsTable(track.lyrics);
    }
}

function addNewLyricLine() {
    const track = state.tracks.find(t => t.id === state.activeTrackId);
    if (!track) return;
    const curTime = state.audioPlayer.currentTime || 0;
    const text = prompt("Masukkan baris lirik baru:", "Lirik baru");
    if (text && text.trim()) {
        const words = text.trim().split(/\s+/).map((w, i) => ({
            word: w,
            start: curTime + (i * 0.4),
            end: curTime + ((i + 1) * 0.4)
        }));
        track.lyrics = track.lyrics || [];
        track.lyrics.push({
            start: curTime,
            end: curTime + (words.length * 0.4),
            text: text.trim(),
            words: words
        });
        track.lyrics.sort((a, b) => a.start - b.start);
        renderLyricsTable(track.lyrics);
    }
}

function clearActiveTrackLyrics() {
    if (!confirm("Hapus seluruh lirik lagu ini?")) return;
    const track = state.tracks.find(t => t.id === state.activeTrackId);
    if (track) {
        track.lyrics = [];
        renderLyricsTable([]);
    }
}

function toggleKaraokePlay() {
    const btn = document.getElementById("karaoke-play-toggle");
    if (state.audioPlayer.paused) {
        state.audioPlayer.play();
        state.isPlayingKaraoke = true;
        btn.innerHTML = '<i data-lucide="pause"></i>';
    } else {
        state.audioPlayer.pause();
        state.isPlayingKaraoke = false;
        btn.innerHTML = '<i data-lucide="play"></i>';
    }
    lucide.createIcons();
}

function renderLiveKaraokeDisplay(currentTime) {
    const display = document.getElementById("karaoke-display-line");
    const track = state.tracks.find(t => t.id === state.activeTrackId);
    if (!track || !track.lyrics || track.lyrics.length === 0) {
        return;
    }

    // Find current active line
    const activeLine = track.lyrics.find(l => currentTime >= l.start && currentTime <= l.end);

    if (!activeLine) {
        // Find upcoming line
        const nextLine = track.lyrics.find(l => l.start > currentTime);
        if (nextLine && nextLine.start - currentTime < 4.0) {
            display.innerHTML = `<span style="opacity: 0.6;">🎵 [Siap-siap...]</span><br><span>${escapeHtml(nextLine.text)}</span>`;
        }
        return;
    }

    // Render words with live highlight
    const wordsHtml = activeLine.words.map(w => {
        let cls = "karaoke-word";
        if (currentTime >= w.end) {
            cls += " passed";
        } else if (currentTime >= w.start && currentTime < w.end) {
            cls += " active";
        }
        return `<span class="${cls}">${escapeHtml(w.word)}</span>`;
    }).join(" ");

    display.innerHTML = wordsHtml;
}

function applyPresetTheme(themeKey) {
    const themes = {
        golden: { normal: "#FFFFFF", singing: "#FFD700", outline: "#000000" },
        neon_cyan: { normal: "#FFFFFF", singing: "#00F0FF", outline: "#0A192F" },
        sunset_pink: { normal: "#FFF0F5", singing: "#FF2A85", outline: "#1A0010" },
        emerald: { normal: "#F0FFF0", singing: "#39FF14", outline: "#051A05" },
        classic: { normal: "#FFFFFF", singing: "#1E90FF", outline: "#000000" }
    };
    if (themeKey === "custom") return;

    const t = themes[themeKey];
    if (t) {
        state.styleConfig.theme = themeKey;
        state.styleConfig.normal_color = t.normal;
        state.styleConfig.singing_color = t.singing;
        state.styleConfig.outline_color = t.outline;

        document.getElementById("color-normal").value = t.normal;
        document.getElementById("color-singing").value = t.singing;
        document.getElementById("color-outline").value = t.outline;
        document.documentElement.style.setProperty('--accent-gold', t.singing);
    }
}

function closePasteModal() {
    document.getElementById("paste-modal").classList.add("hidden");
}

// ================= 5. RENDER & EXPORT =================
function initRenderExport() {
    const btnStart = document.getElementById("btn-start-render");
    if (btnStart) btnStart.addEventListener("click", startRenderProcess);

    const btnOpenResult = document.getElementById("btn-open-result-folder");
    if (btnOpenResult) btnOpenResult.addEventListener("click", openOutputsFolder);

    const btnOpenFolder = document.getElementById("btn-open-folder");
    if (btnOpenFolder) btnOpenFolder.addEventListener("click", openOutputsFolder);

    // YouTube Timestamps copy & download buttons
    const btnCopyTimestamps = document.getElementById("btn-copy-timestamps");
    if (btnCopyTimestamps) {
        btnCopyTimestamps.addEventListener("click", () => {
            const box = document.getElementById("youtube-timestamps-box");
            if (!box || !box.value.trim()) {
                alert("Daftar lagu masih kosong. Tambahkan lagu di Langkah 2 terlebih dahulu.");
                return;
            }
            navigator.clipboard.writeText(box.value).then(() => {
                const origHtml = btnCopyTimestamps.innerHTML;
                btnCopyTimestamps.innerHTML = `<i data-lucide="check"></i> Tersalin!`;
                btnCopyTimestamps.classList.add("btn-primary");
                lucide.createIcons();
                setTimeout(() => {
                    btnCopyTimestamps.innerHTML = origHtml;
                    btnCopyTimestamps.classList.remove("btn-primary");
                    lucide.createIcons();
                }, 2000);
            }).catch(() => {
                box.select();
                document.execCommand("copy");
                alert("Timestamps YouTube berhasil disalin ke clipboard!");
            });
        });
    }

    const btnDownloadTimestamps = document.getElementById("btn-download-timestamps-txt");
    if (btnDownloadTimestamps) {
        btnDownloadTimestamps.addEventListener("click", () => {
            const box = document.getElementById("youtube-timestamps-box");
            if (!box || !box.value.trim()) {
                alert("Daftar lagu masih kosong. Tambahkan lagu di Langkah 2 terlebih dahulu.");
                return;
            }
            const blob = new Blob([box.value], { type: "text/plain;charset=utf-8" });
            const url = URL.createObjectURL(blob);
            const a = document.createElement("a");
            a.href = url;
            a.download = "youtube_timestamps.txt";
            document.body.appendChild(a);
            a.click();
            document.body.removeChild(a);
            URL.revokeObjectURL(url);
        });
    }

    // Auto-detect and configure universal NVIDIA GPU / CUDA or CPU
    loadHardwareCapabilities();

    // Visual effect selector in Step 4
    const renderVeSelect = document.getElementById("render-visual-effect-select");
    if (renderVeSelect) {
        renderVeSelect.addEventListener("change", (e) => {
            state.visualEffect = e.target.value;
            const imgVe = document.getElementById("img-visual-effect-select");
            if (imgVe) imgVe.value = e.target.value;
            updateRenderSummary();
        });
    }
}

async function loadHardwareCapabilities() {
    try {
        const res = await fetch("/api/hardware");
        if (!res.ok) return;
        const hw = await res.json();
        state.hardware = hw;

        const nameEl = document.getElementById("gpu-name-display");
        const descEl = document.getElementById("gpu-status-desc");
        const encoderSelect = document.getElementById("render-encoder-select");
        const whisperDevice = document.getElementById("whisper-device-select");
        const hwaccelLabel = document.getElementById("render-hwaccel-label");

        if (hw.has_nvidia) {
            const gpuName = hw.gpu_name || "NVIDIA GPU";
            if (nameEl) nameEl.textContent = `⚡ ${gpuName}`;
            if (descEl) descEl.textContent = `Akselerasi Hardware CUDA & NVENC Aktif (${gpuName})`;
            if (hwaccelLabel) hwaccelLabel.textContent = `Akselerasi Penuh GPU (${gpuName})`;
            if (encoderSelect) {
                encoderSelect.value = "nvenc_h264";
                if (encoderSelect.options[0]) encoderSelect.options[0].textContent = `NVIDIA NVENC H.264 (${gpuName} - Super Cepat)`;
                if (encoderSelect.options[1]) encoderSelect.options[1].textContent = `NVIDIA NVENC H.265 / HEVC (${gpuName} - Kompresi Tinggi)`;
            }
            if (whisperDevice) whisperDevice.value = "cuda";
            const teleEncoder = document.getElementById("tele-encoder");
            if (teleEncoder) teleEncoder.textContent = `NVIDIA NVENC (${gpuName})`;
        } else {
            if (nameEl) nameEl.textContent = "GPU NVIDIA Tidak Ditemukan";
            if (descEl) descEl.textContent = "Menggunakan CPU Software Encoder (Mode Universal Semua PC/Laptop)";
            if (hwaccelLabel) hwaccelLabel.textContent = "Mode CPU Universal (libx264)";
            if (encoderSelect) encoderSelect.value = "cpu_x264";
            if (whisperDevice) whisperDevice.value = "cpu";
            const teleEncoder = document.getElementById("tele-encoder");
            if (teleEncoder) teleEncoder.textContent = "CPU Software (libx264)";
        }
    } catch (e) {
        console.error("Gagal memeriksa hardware:", e);
    }
}

function updateRenderSummary() {
    document.getElementById("sum-video-name").textContent = state.video ? state.video.filename : "Belum dipilih";

    const tracksWithLyrics = state.tracks.filter(t => t.lyrics && t.lyrics.length > 0).length;
    document.getElementById("sum-tracks-count").textContent = `${state.tracks.length} Lagu (${tracksWithLyrics} ada lirik karaoke)`;

    const totalSec = state.tracks.reduce((a, t) => a + (t.duration || 0), 0);
    document.getElementById("sum-total-duration").textContent = formatSec(totalSec);

    if (state.video && state.video.duration > 0) {
        const loops = Math.ceil(totalSec / state.video.duration);
        document.getElementById("sum-loop-times").textContent = `${loops}x Putaran`;
    } else {
        document.getElementById("sum-loop-times").textContent = "-";
    }

    const encoderSelect = document.getElementById("render-encoder-select");
    const encName = encoderSelect ? encoderSelect.options[encoderSelect.selectedIndex].text.split("(")[0].trim() : "NVIDIA NVENC";
    document.getElementById("sum-karaoke-theme").textContent = `${state.styleConfig.theme} • ${encName}`;

    // Update visual effect summary
    const veNames = {
        none: "Tanpa Efek (Asli)",
        rain: "🌧️ Hujan (Rain)",
        snow: "❄️ Salju (Snow)",
        fire: "🔥 Bara Api (Embers)",
        bokeh: "✨ Bokeh Sinematik",
        light_leak: "🌅 Light Leak",
        color_grading: "🎨 Color Grading (Teal & Orange)",
        vhs: "📼 VHS Retro",
        glitch: "⚡ Glitch Digital"
    };
    const sumVe = document.getElementById("sum-visual-effect");
    if (sumVe) {
        const renderVeSelect = document.getElementById("render-visual-effect-select");
        const curVe = (renderVeSelect && renderVeSelect.value !== "none")
            ? renderVeSelect.value
            : (state.visualEffect || (state.video && state.video.visual_effect) || "none");
        sumVe.textContent = veNames[curVe] || curVe;
    }

    // Update YouTube timestamps preview
    updateYouTubeTimestampsUI();
}

async function startRenderProcess() {
    if (!state.video) {
        alert("Video background belum dipilih.");
        return;
    }
    if (state.tracks.length === 0) {
        alert("Daftar lagu masih kosong.");
        return;
    }

    const tracksWithoutLyrics = state.tracks.filter(t => !t.lyrics || t.lyrics.length === 0);
    if (tracksWithoutLyrics.length > 0) {
        const proceed = confirm(`Perhatian: Terdapat ${tracksWithoutLyrics.length} dari ${state.tracks.length} lagu yang belum memiliki lirik karaoke.\n\nKlik 'OK' jika ingin tetap merender video (hanya menampilkan intro judul lagu tanpa lirik berjalan), atau 'Cancel' untuk kembali ke Langkah 3 dan klik tombol 'Buat Auto Lirik AI Sekarang'.`);
        if (!proceed) return;
    }

    const resolution = document.getElementById("render-res-select").value;
    const fps = parseInt(document.getElementById("render-fps-select").value);
    const encoder = document.getElementById("render-encoder-select") ? document.getElementById("render-encoder-select").value : "nvenc_h264";
    const preset = document.getElementById("render-preset-select") ? document.getElementById("render-preset-select").value : "p4";
    const quality_cq = document.getElementById("render-quality-select") ? parseInt(document.getElementById("render-quality-select").value) : 22;
    const use_hwaccel = document.getElementById("render-hwaccel-toggle") ? document.getElementById("render-hwaccel-toggle").checked : true;

    const renderVeSelect = document.getElementById("render-visual-effect-select");
    const chosenVisualEffect = (renderVeSelect && renderVeSelect.value !== "none")
        ? renderVeSelect.value
        : (state.visualEffect || (state.video && state.video.visual_effect) || "none");

    const btnStart = document.getElementById("btn-start-render");
    btnStart.disabled = true;

    // UI state transition
    document.getElementById("render-idle-prompt").classList.add("hidden");
    document.getElementById("render-result-box").classList.add("hidden");
    const progressBox = document.getElementById("render-progress-box");
    progressBox.classList.remove("hidden");

    const badge = document.getElementById("render-status-badge");
    badge.textContent = "Sedang Dirender (GPU)";
    badge.className = "badge badge-primary";

    const payload = {
        video_path: state.video.path,
        tracks: state.tracks,
        aspect_ratio: state.aspectRatio,
        resolution: resolution,
        fit_mode: state.fitMode,
        fps: fps,
        video_volume: state.muteVideo ? 0.0 : 1.0,
        crossfade_duration: 0.0,
        style_config: state.styleConfig,
        encoder: encoder,
        preset: preset,
        quality_cq: quality_cq,
        use_hwaccel: use_hwaccel,
        beat_pulse: state.imageBeatPulse !== undefined ? state.imageBeatPulse : true,
        is_from_image: state.video.is_from_image || false,
        source_image_path: state.video.source_image_path || null,
        image_effect: (document.getElementById("img-effect-select") ? document.getElementById("img-effect-select").value : "zoom_in"),
        visual_effect: chosenVisualEffect
    };

    try {
        const res = await fetch("/api/render", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify(payload)
        });
        const data = await res.json();
        if (!res.ok) throw new Error(data.detail || "Gagal memulai render.");

        trackRenderProgress(data.job_id);
    } catch (err) {
        alert(err.message);
        btnStart.disabled = false;
        badge.textContent = "Gagal";
        badge.className = "badge badge-dim";
    }
}

function trackRenderProgress(jobId) {
    if (state.renderInterval) clearInterval(state.renderInterval);

    state.renderInterval = setInterval(async () => {
        try {
            const res = await fetch(`/api/render/status/${jobId}`);
            if (!res.ok) return;
            const job = await res.json();

            // Update Progress bar
            const percent = job.progress_percent || 0;
            document.getElementById("render-progress-fill").style.width = `${percent}%`;
            document.getElementById("render-progress-percent").textContent = `${percent.toFixed(1)}%`;
            document.getElementById("render-progress-time").textContent = `${formatSec(job.current_time_sec)} / ${formatSec(job.total_time_sec)}`;

            // Telemetry
            if (job.encoder_name && document.getElementById("tele-encoder")) {
                document.getElementById("tele-encoder").textContent = job.encoder_name;
            }
            document.getElementById("tele-speed").textContent = job.speed || "0x";
            document.getElementById("tele-fps").textContent = `${job.fps || 0} fps`;
            document.getElementById("tele-eta").textContent = job.eta_seconds > 0 ? `${job.eta_seconds}s` : "Hampir selesai";

            if (job.status === "completed") {
                clearInterval(state.renderInterval);
                onRenderCompleted(job);
            } else if (job.status === "failed") {
                clearInterval(state.renderInterval);
                alert(`Render gagal: ${job.error_message}`);
                document.getElementById("btn-start-render").disabled = false;
            }
        } catch (e) {
            console.error(e);
        }
    }, 800);
}

function onRenderCompleted(job) {
    const badge = document.getElementById("render-status-badge");
    badge.textContent = "Selesai 100%";
    badge.className = "badge badge-success";

    document.getElementById("render-progress-box").classList.add("hidden");
    const resultBox = document.getElementById("render-result-box");
    resultBox.classList.remove("hidden");

    const player = document.getElementById("final-video-player");
    player.src = job.download_url;
    player.load();

    const dlBtn = document.getElementById("btn-download-video");
    dlBtn.href = job.download_url;

    document.getElementById("btn-start-render").disabled = false;
    lucide.createIcons();
}

async function openOutputsFolder() {
    try {
        await fetch("/api/open-folder", { method: "POST" });
    } catch (e) {
        console.error(e);
    }
}

// ================= 6. QUICK DEMO GENERATOR =================
function initQuickDemo() {
    const demoBtn = document.getElementById("btn-sample-demo");
    if (demoBtn) {
        demoBtn.addEventListener("click", generateSampleDemo);
    }
}

async function generateSampleDemo() {
    const demoBtn = document.getElementById("btn-sample-demo");
    const orig = demoBtn.innerHTML;
    demoBtn.disabled = true;
    demoBtn.innerHTML = `<i data-lucide="loader-2" class="spin"></i> Membuat Sampel...`;
    lucide.createIcons();

    try {
        const res = await fetch("/api/sample-demo", { method: "POST" });
        const data = await res.json();
        if (!res.ok) throw new Error(data.detail || "Gagal membuat demo.");

        state.video = data.video;
        if (data.tracks) {
            data.tracks.forEach(t => {
                if (t.title) t.title = cleanProperTitle(t.title);
            });
        }
        state.tracks = data.tracks;
        state.activeTrackId = data.tracks[0].id;

        displayVideoPreview(data.video);
        if (state.shufflePlaylist && state.tracks.length > 1) {
            shufflePlaylist(false);
        } else {
            renderPlaylist();
            calculateLoopStats();
            updateTrackSelectDropdown();
            updateYouTubeTimestampsUI();
        }

        alert("Sampel Video Background dan 2 Lagu dengan Auto Lirik Karaoke berhasil disiapkan! Anda bisa langsung ke langkah Render atau melihat lirik.");
    } catch (err) {
        alert(err.message);
    } finally {
        demoBtn.disabled = false;
        demoBtn.innerHTML = orig;
        lucide.createIcons();
    }
}

// ================= HELPERS =================
function formatSec(seconds) {
    const s = Math.round(seconds || 0);
    const m = Math.floor(s / 60);
    const sec = s % 60;
    return `${m.toString().padStart(2, '0')}:${sec.toString().padStart(2, '0')}`;
}

function escapeHtml(str) {
    if (!str) return '';
    return str
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;")
        .replace(/"/g, "&quot;")
        .replace(/'/g, "&#039;");
}

function cleanProperTitle(rawText) {
    if (!rawText) return "";
    let text = String(rawText).trim();

    // 1. Strip file extension
    text = text.replace(/\.(mp3|wav|ogg|flac|m4a|aac|opus|mp4|mkv|webm)$/i, "");

    // 2. Remove hash prefixes like 'audio_0cbae9d7_' or 'video_bb2e44bd_'
    text = text.replace(/^(audio|video)_[0-9a-zA-Z]+_/i, "");
    text = text.replace(/^(track|audio|video|song|music)[_\-\s]*\d*[_\-\s]*/i, "");

    // 3. Remove leading track numbering like '01. ', '1 - ', '01 '
    text = text.replace(/^\d+[\.\-\s]+/, "");

    // 4. Replace underscores with spaces early
    text = text.replace(/_+/g, " ");

    // 5. Remove copy / duplicate / media tags like (1), [1], (Official Audio), [Lyrics], etc.
    text = text.replace(/[\(\[\{]\s*\d+\s*[\)\]\}]/g, "");
    text = text.replace(/[\(\[\{]\s*(?:official\s*(?:video|audio|music|lyrics?)?|lyrics?|audio|video|remaster(?:ed)?)\s*[\)\]\}]/gi, "");
    text = text.replace(/[\s\-\(\[]+(?:copy|official|lyrics?|audio)[\s\-\)\]]*$/i, "");

    // 6. Remove 'by ...' if title contains artist suffix like 'Song by Artist' or dangling 'by'
    text = text.replace(/\s+by\s+.*$/i, "");
    text = text.replace(/\s+by$/i, "");

    // 7. Replace hyphens and dashes with spaces
    text = text.replace(/\s*-\s*/g, " ");
    text = text.replace(/(?<=[a-zA-Z0-9])-(?=[a-zA-Z0-9])/g, " ");

    // 8. Normalize multiple spaces
    text = text.replace(/\s+/g, " ").trim();

    // 9. Convert to proper TitleCase / CamelCase words
    const words = text.split(" ");
    const capitalizedWords = [];
    for (const w of words) {
        if (!w) continue;
        if (w.length > 1 && w === w.toUpperCase() && w.length <= 4) {
            capitalizedWords.push(w);
        } else {
            capitalizedWords.push(w.charAt(0).toUpperCase() + w.slice(1).toLowerCase());
        }
    }
    const res = capitalizedWords.join(" ").trim();
    return res || String(rawText).trim();
}

function formatTimestampTime(seconds) {
    const totalSec = Math.round(seconds || 0);
    const hours = Math.floor(totalSec / 3600);
    const mins = Math.floor((totalSec % 3600) / 60);
    const secs = totalSec % 60;
    if (hours > 0) {
        return `${String(hours).padStart(2, '0')}:${String(mins).padStart(2, '0')}:${String(secs).padStart(2, '0')}`;
    }
    return `${String(mins).padStart(2, '0')}:${String(secs).padStart(2, '0')}`;
}

function updateYouTubeTimestampsUI() {
    const box = document.getElementById("youtube-timestamps-box");
    const countLabel = document.getElementById("timestamps-track-count");
    if (!box) return;

    if (!state.tracks || state.tracks.length === 0) {
        box.value = "";
        if (countLabel) countLabel.textContent = "0 Lagu terdaftar";
        return;
    }

    let currentSec = 0;
    const lines = state.tracks.map((track, idx) => {
        const timeStr = formatTimestampTime(currentSec);
        const title = cleanProperTitle(track.title || track.filename || `Lagu ${idx + 1}`);
        currentSec += (track.duration || 0);
        return `${timeStr} ${title}`;
    });

    box.value = lines.join("\n");
    if (countLabel) {
        countLabel.textContent = `${state.tracks.length} Lagu terdaftar (${formatSec(currentSec)})`;
    }
}

// ================= 7. STORAGE & CACHE MANAGER =================
function initStorageManager() {
    const btnStorage = document.getElementById("btn-open-storage");
    if (btnStorage) {
        btnStorage.addEventListener("click", openStorageModal);
    }
}

async function openStorageModal() {
    document.getElementById("storage-modal").classList.remove("hidden");
    await loadStorageStats();
    lucide.createIcons();
}

function closeStorageModal() {
    document.getElementById("storage-modal").classList.add("hidden");
}

async function loadStorageStats() {
    try {
        const res = await fetch("/api/storage/stats");
        if (!res.ok) return;
        const data = await res.json();

        // 1. Uploads
        document.getElementById("storage-uploads-size").textContent = `${data.uploads.size_mb} MB`;
        document.getElementById("storage-uploads-count").textContent = `${data.uploads.count} file tersimpan`;

        // 2. Outputs
        document.getElementById("storage-outputs-size").textContent = `${data.outputs.size_mb} MB`;
        document.getElementById("storage-outputs-count").textContent = `${data.outputs.count} video selesai`;

        // 3. AI Cache
        document.getElementById("storage-ai-size").textContent = `${data.hf_cache.size_mb} MB`;
        const modelNames = data.hf_cache.models.map(m => m.replace("models--Systran--faster-whisper-", "")).join(", ");
        document.getElementById("storage-ai-info").textContent = modelNames ? `Model: ${modelNames}` : "Disimpan agar 100% offline";

        // 4. File list
        renderStorageFilesList(data.outputs.files || []);
    } catch (e) {
        console.error("Gagal memuat statistik penyimpanan:", e);
    }
}

function renderStorageFilesList(files) {
    const container = document.getElementById("storage-files-list");
    if (!files || files.length === 0) {
        container.innerHTML = `
            <div style="text-align: center; padding: 20px; color: var(--text-muted); font-size: 12px;">
                Belum ada video hasil render yang tersimpan.
            </div>
        `;
        return;
    }

    container.innerHTML = files.map(f => `
        <div class="storage-file-row">
            <div class="storage-file-info">
                <i data-lucide="video" style="width: 16px; height: 16px; flex-shrink: 0; color: var(--accent-cyan);"></i>
                <span title="${escapeHtml(f.name)}"><strong>${escapeHtml(f.name)}</strong> (${f.size_mb} MB • ${f.modified_str})</span>
            </div>
            <div class="storage-file-actions">
                <a href="/outputs/${escapeHtml(f.name)}" download class="btn-icon-sm" title="Download video">
                    <i data-lucide="download"></i>
                </a>
                <button class="btn-icon-sm text-danger" onclick="deleteOutputFile('${escapeHtml(f.name)}')" title="Hapus video ini">
                    <i data-lucide="trash-2"></i>
                </button>
            </div>
        </div>
    `).join("");

    lucide.createIcons();
}

async function cleanStorageCategory(category) {
    let confirmMsg = "";
    let payload = { clean_uploads: false, clean_outputs: false, clean_temp_only: false };

    if (category === "uploads") {
        confirmMsg = "Hapus semua file video & musik sementara di folder Uploads? (Sesi upload saat ini akan direset)";
        payload.clean_uploads = true;
    } else if (category === "outputs") {
        confirmMsg = "Hapus semua video hasil render yang telah selesai?";
        payload.clean_outputs = true;
    } else if (category === "temp") {
        confirmMsg = "Bersihkan berkas sampah subtitle (.ass) dan daftar sementara yang tidak terpakai?";
        payload.clean_temp_only = true;
    }

    if (confirmMsg && !confirm(confirmMsg)) return;

    try {
        const res = await fetch("/api/storage/clean", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify(payload)
        });
        const data = await res.json();
        if (!res.ok) throw new Error("Gagal membersihkan penyimpanan.");

        if (category === "uploads") {
            state.video = null;
            state.tracks = [];
            calculateLoopStats();
            document.getElementById("video-dropzone").classList.remove("hidden");
            document.getElementById("video-preview-wrapper").classList.add("hidden");
            document.getElementById("video-badge").textContent = "Belum ada video";
            document.getElementById("video-badge").className = "badge badge-dim";
            renderPlaylist();
        }

        await loadStorageStats();
        alert("Penyimpanan berhasil dibersihkan!");
    } catch (e) {
        alert(e.message);
    }
}

async function deleteOutputFile(filename) {
    if (!confirm(`Hapus video "${filename}"?`)) return;

    try {
        const res = await fetch("/api/storage/delete-file", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ folder: "outputs", filename: filename })
        });
        if (!res.ok) throw new Error("Gagal menghapus file.");

        await loadStorageStats();
    } catch (e) {
        alert(e.message);
    }
}


// ================= 8. THUMBNAIL GENERATOR =================
let offscreenVideoThumb = null;

function initThumbnailGenerator() {
    const titleInput = document.getElementById("thumb-title-input");
    const subInput = document.getElementById("thumb-subtitle-input");
    const colModeSelect = document.getElementById("thumb-column-mode");
    const numStyleSelect = document.getElementById("thumb-number-style");
    const tracksArea = document.getElementById("thumb-tracks-textarea");
    const titleSizeSlider = document.getElementById("thumb-title-size");
    const trackSizeSlider = document.getElementById("thumb-track-size");
    const titleFontSelect = document.getElementById("thumb-font-title-family");
    const subFontSelect = document.getElementById("thumb-font-sub-family");
    const colorTitle = document.getElementById("thumb-color-title");
    const colorSub = document.getElementById("thumb-color-subtitle");
    const vigSlider = document.getElementById("thumb-vignette-slider");

    if (!titleInput) return;

    titleInput.addEventListener("input", (e) => {
        state.thumb.title = e.target.value;
        renderThumbnailCanvas();
    });

    subInput.addEventListener("input", (e) => {
        state.thumb.subtitle = e.target.value;
        renderThumbnailCanvas();
    });

    colModeSelect.addEventListener("change", (e) => {
        state.thumb.columnMode = e.target.value;
        renderThumbnailCanvas();
    });

    numStyleSelect.addEventListener("change", (e) => {
        state.thumb.numberStyle = e.target.value;
        renderThumbnailCanvas();
    });

    tracksArea.addEventListener("input", (e) => {
        const lines = e.target.value.split("\n").map(l => l.trim()).filter(l => l.length > 0);
        state.thumb.tracks = lines;
        document.getElementById("thumb-track-count").textContent = lines.length;
        renderThumbnailCanvas();
    });

    titleSizeSlider.addEventListener("input", (e) => {
        state.thumb.titleSize = parseInt(e.target.value);
        document.getElementById("val-thumb-title-size").textContent = e.target.value;
        renderThumbnailCanvas();
    });

    trackSizeSlider.addEventListener("input", (e) => {
        state.thumb.trackSize = parseInt(e.target.value);
        document.getElementById("val-thumb-track-size").textContent = e.target.value;
        renderThumbnailCanvas();
    });

    titleFontSelect.addEventListener("change", (e) => {
        state.thumb.titleFont = e.target.value;
        renderThumbnailCanvas();
    });

    subFontSelect.addEventListener("change", (e) => {
        state.thumb.subFont = e.target.value;
        renderThumbnailCanvas();
    });

    colorTitle.addEventListener("input", (e) => {
        state.thumb.titleColor = e.target.value;
        renderThumbnailCanvas();
    });

    colorSub.addEventListener("input", (e) => {
        state.thumb.subtitleColor = e.target.value;
        renderThumbnailCanvas();
    });

    vigSlider.addEventListener("input", (e) => {
        state.thumb.vignette = parseInt(e.target.value);
        renderThumbnailCanvas();
    });

    document.getElementById("btn-thumb-sync-playlist").addEventListener("click", () => {
        syncTracksToThumbnail(true);
    });

    document.getElementById("btn-thumb-download").addEventListener("click", downloadThumbnailImage);
    document.getElementById("btn-thumb-save-folder").addEventListener("click", saveThumbnailToServer);

    // Frame scrubber
    const timeSlider = document.getElementById("thumb-time-slider");
    timeSlider.addEventListener("input", (e) => {
        seekThumbnailVideoFrame(parseFloat(e.target.value));
    });

    document.getElementById("btn-thumb-capture-current").addEventListener("click", () => {
        const v = document.getElementById("video-player");
        if (v && v.src) {
            seekThumbnailVideoFrame(v.currentTime);
        } else {
            alert("Belum ada video yang dimuat.");
        }
    });

    // Custom background upload
    const bgInput = document.getElementById("thumb-bg-file-input");
    bgInput.addEventListener("change", (e) => {
        const file = e.target.files[0];
        if (!file) return;
        const reader = new FileReader();
        reader.onload = (event) => {
            const img = new Image();
            img.onload = () => {
                state.thumb.backgroundImg = img;
                renderThumbnailCanvas();
            };
            img.src = event.target.result;
        };
        reader.readAsDataURL(file);
    });
}

function onEnterThumbnailStep() {
    syncTracksToThumbnail(false);
    setupThumbnailVideoSource();
    renderThumbnailCanvas();
}

function prepareThumbnailFromRender() {
    const finalPlayer = document.getElementById("final-video-player");
    if (finalPlayer && finalPlayer.src) {
        state.thumb.activeVideoUrl = finalPlayer.src;
    }
    syncTracksToThumbnail(true);
    goToStep(5);
}

function syncTracksToThumbnail(force = false) {
    const area = document.getElementById("thumb-tracks-textarea");
    if (!area) return;

    if (force || area.value.trim() === "") {
        if (state.tracks.length > 0) {
            const lines = state.tracks.map((t, idx) => {
                let clean = cleanProperTitle(t.title || t.filename || "");
                return clean || `Lagu ${idx + 1}`;
            });
            area.value = lines.join("\n");
            state.thumb.tracks = lines;
            document.getElementById("thumb-track-count").textContent = lines.length;
        } else if (area.value.trim() === "") {
            // Sample worship song titles if playlist is empty
            const demoTracks = [
                "Living Hope", "Goodness Of God", "Way Maker", "I Speak Jesus",
                "10,000 Reasons", "What A Beautiful Name", "Lord, I Need You",
                "I Surrender", "Oceans", "Good Good Father",
                "King Of Kings", "Holy Forever", "In Control", "House Of The Lord",
                "Hosanna", "Raise A Hallelujah", "Here I Am To Worship", "Do It Again",
                "Breathe", "This I Believe"
            ];
            area.value = demoTracks.join("\n");
            state.thumb.tracks = demoTracks;
            document.getElementById("thumb-track-count").textContent = demoTracks.length;
        }
    }
    renderThumbnailCanvas();
}

function setupThumbnailVideoSource() {
    let videoUrl = state.thumb.activeVideoUrl;
    if (!videoUrl && state.video && state.video.web_url) {
        videoUrl = state.video.web_url;
    }

    if (!videoUrl) return;

    if (!offscreenVideoThumb) {
        offscreenVideoThumb = document.createElement("video");
        offscreenVideoThumb.crossOrigin = "anonymous";
        offscreenVideoThumb.muted = true;
        offscreenVideoThumb.playsInline = true;

        offscreenVideoThumb.addEventListener("loadedmetadata", () => {
            const duration = offscreenVideoThumb.duration || 60;
            const slider = document.getElementById("thumb-time-slider");
            slider.max = duration;
            document.getElementById("thumb-time-display").textContent = `00:00 / ${formatSec(duration)}`;
            seekThumbnailVideoFrame(0.5);
        });

        offscreenVideoThumb.addEventListener("seeked", () => {
            state.thumb.backgroundImg = offscreenVideoThumb;
            renderThumbnailCanvas();
        });
    }

    if (offscreenVideoThumb.src !== window.location.origin + videoUrl && offscreenVideoThumb.src !== videoUrl) {
        offscreenVideoThumb.src = videoUrl;
        offscreenVideoThumb.load();
    }
}

function seekThumbnailVideoFrame(sec) {
    if (!offscreenVideoThumb) return;
    const s = Math.max(0, Math.min(sec, offscreenVideoThumb.duration || 60));
    offscreenVideoThumb.currentTime = s;
    const dur = offscreenVideoThumb.duration || 60;
    document.getElementById("thumb-time-display").textContent = `${formatSec(s)} / ${formatSec(dur)}`;
    document.getElementById("thumb-time-slider").value = s;
}

function renderThumbnailCanvas() {
    const canvas = document.getElementById("thumbnail-canvas");
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    const width = 1920;
    const height = 1080;

    ctx.clearRect(0, 0, width, height);

    // 1. Draw Background
    if (state.thumb.backgroundImg) {
        const img = state.thumb.backgroundImg;
        const imgW = img.videoWidth || img.width;
        const imgH = img.videoHeight || img.height;
        if (imgW && imgH) {
            const hRatio = width / imgW;
            const vRatio = height / imgH;
            const ratio = Math.max(hRatio, vRatio);
            const shiftX = (width - imgW * ratio) / 2;
            const shiftY = (height - imgH * ratio) / 2;
            ctx.drawImage(img, 0, 0, imgW, imgH, shiftX, shiftY, imgW * ratio, imgH * ratio);
        }
    } else {
        // Fallback concert dark background
        const bgGrad = ctx.createLinearGradient(0, 0, width, height);
        bgGrad.addColorStop(0, "#080b12");
        bgGrad.addColorStop(0.5, "#161c28");
        bgGrad.addColorStop(1, "#07090e");
        ctx.fillStyle = bgGrad;
        ctx.fillRect(0, 0, width, height);
    }

    // 2. Draw Dark Vignette Edge Gradients
    const vig = (state.thumb.vignette || 55) / 100.0;
    if (vig > 0) {
        // Left gradient
        const leftGrad = ctx.createLinearGradient(0, 0, width * 0.45, 0);
        leftGrad.addColorStop(0, `rgba(0, 0, 0, ${vig * 0.95})`);
        leftGrad.addColorStop(1, "rgba(0, 0, 0, 0)");
        ctx.fillStyle = leftGrad;
        ctx.fillRect(0, 0, width * 0.45, height);

        // Right gradient
        const rightGrad = ctx.createLinearGradient(width, 0, width * 0.55, 0);
        rightGrad.addColorStop(0, `rgba(0, 0, 0, ${vig * 0.95})`);
        rightGrad.addColorStop(1, "rgba(0, 0, 0, 0)");
        ctx.fillStyle = rightGrad;
        ctx.fillRect(width * 0.55, 0, width * 0.45, height);

        // Top gradient
        const topGrad = ctx.createLinearGradient(0, 0, 0, height * 0.32);
        topGrad.addColorStop(0, `rgba(0, 0, 0, ${vig * 0.90})`);
        topGrad.addColorStop(1, "rgba(0, 0, 0, 0)");
        ctx.fillStyle = topGrad;
        ctx.fillRect(0, 0, width, height * 0.32);

        // Bottom gradient
        const botGrad = ctx.createLinearGradient(0, height, 0, height * 0.72);
        botGrad.addColorStop(0, `rgba(0, 0, 0, ${vig * 0.85})`);
        botGrad.addColorStop(1, "rgba(0, 0, 0, 0)");
        ctx.fillStyle = botGrad;
        ctx.fillRect(0, height * 0.72, width, height * 0.28);
    }

    // Text Drawer Helper
    function drawHighContrastText(text, x, y, font, fill, stroke, strokeW, align = "left") {
        ctx.save();
        ctx.font = font;
        ctx.textAlign = align;
        ctx.textBaseline = "middle";

        // Shadow
        ctx.shadowColor = "#000000";
        ctx.shadowBlur = 10;
        ctx.shadowOffsetX = 3;
        ctx.shadowOffsetY = 4;

        // Outline
        ctx.strokeStyle = stroke;
        ctx.lineWidth = strokeW;
        ctx.lineJoin = "round";
        ctx.miterLimit = 2;
        ctx.strokeText(text, x, y);

        // Foreground
        ctx.shadowColor = "transparent";
        ctx.fillStyle = fill;
        ctx.fillText(text, x, y);
        ctx.restore();
    }

    // 3. Draw Main Title (Top Center)
    const titleText = (state.thumb.title || "").trim();
    if (titleText) {
        const titleSize = state.thumb.titleSize || 130;
        const titleFont = state.thumb.titleFont || "Impact";
        drawHighContrastText(
            titleText,
            width / 2,
            120,
            `bold ${titleSize}px "${titleFont}", "Bebas Neue", sans-serif`,
            state.thumb.titleColor || "#FFFFFF",
            "#000000",
            7,
            "center"
        );
    }

    // 4. Tracklist Formatting & Layout
    const area = document.getElementById("thumb-tracks-textarea");
    const rawLines = area ? area.value.split("\n") : state.thumb.tracks;
    const cleanTracks = rawLines.map(l => l.replace(/^\d+[\.\-\s]+/, "").trim()).filter(l => l.length > 0);

    let isTwoCols = false;
    if (state.thumb.columnMode === "2") {
        isTwoCols = true;
    } else if (state.thumb.columnMode === "1") {
        isTwoCols = false;
    } else {
        // Auto mode
        isTwoCols = cleanTracks.length > 10;
    }

    const badge = document.getElementById("thumb-mode-badge");
    if (badge) {
        badge.textContent = isTwoCols ? "2 Kolom (Kiri & Kanan)" : "1 Kolom (Kiri Saja)";
    }

    const trackSize = state.thumb.trackSize || 36;
    const trackFont = `bold ${trackSize}px "Arial", "Plus Jakarta Sans", sans-serif`;
    const trackColor = state.thumb.titleColor || "#FFFFFF";
    const startY = 260;
    const lineSpacing = trackSize * 1.58;
    const maxRows = 11;

    if (!isTwoCols) {
        // SINGLE COLUMN (LEFT SIDE ONLY)
        const leftX = 100;
        const tracksToShow = cleanTracks.slice(0, 14);
        tracksToShow.forEach((title, i) => {
            const numStr = String(i + 1).padStart(2, "0") + ".";
            let displayTitle = title;
            if (displayTitle.length > 30) displayTitle = displayTitle.substring(0, 28) + "...";
            const textLine = `${numStr}  ${displayTitle}`;
            const y = startY + (i * lineSpacing);
            if (y < height - 100) {
                drawHighContrastText(textLine, leftX, y, trackFont, trackColor, "#000000", 4, "left");
            }
        });
    } else {
        // TWO COLUMNS (LEFT & RIGHT)
        const half = Math.ceil(cleanTracks.length / 2);
        const leftTracks = cleanTracks.slice(0, half).slice(0, maxRows);
        const rightTracks = cleanTracks.slice(half).slice(0, maxRows);

        // Left Column (01. Title)
        const leftX = 100;
        leftTracks.forEach((title, i) => {
            const numStr = String(i + 1).padStart(2, "0") + ".";
            let displayTitle = title;
            if (displayTitle.length > 25) displayTitle = displayTitle.substring(0, 23) + "...";
            const textLine = `${numStr}  ${displayTitle}`;
            const y = startY + (i * lineSpacing);
            drawHighContrastText(textLine, leftX, y, trackFont, trackColor, "#000000", 4, "left");
        });

        // Right Column (Title .01 or 11. Title)
        const rightX = width - 100;
        rightTracks.forEach((title, j) => {
            let displayTitle = title;
            if (displayTitle.length > 25) displayTitle = displayTitle.substring(0, 23) + "...";
            let textLine = "";
            if (state.thumb.numberStyle === "mirrored") {
                // Style in sample image: "King Of Kings  .01"
                const numStr = "." + String(j + 1).padStart(2, "0");
                textLine = `${displayTitle}  ${numStr}`;
            } else {
                // Sequential style: "11.  King Of Kings"
                const numStr = String(half + j + 1).padStart(2, "0") + ".";
                textLine = `${displayTitle}  ${numStr}`;
            }
            const y = startY + (j * lineSpacing);
            drawHighContrastText(textLine, rightX, y, trackFont, trackColor, "#000000", 4, "right");
        });
    }

    // 5. Draw Subtitle / Signature (Bottom Center)
    const subText = (state.thumb.subtitle || "").trim();
    if (subText) {
        const subSize = Math.round((state.thumb.titleSize || 130) * 0.72);
        const subFont = state.thumb.subFont || "Great Vibes";
        drawHighContrastText(
            subText,
            width / 2,
            960,
            `italic ${subSize}px "${subFont}", cursive`,
            state.thumb.subtitleColor || "#FFD700",
            "#000000",
            4,
            "center"
        );
    }
}

function downloadThumbnailImage() {
    const canvas = document.getElementById("thumbnail-canvas");
    if (!canvas) return;

    const titleSlug = (state.thumb.title || "thumbnail").toLowerCase().replace(/[^a-z0-9]/g, "_").substring(0, 30);
    const link = document.createElement("a");
    link.download = `thumbnail_${titleSlug}_${Date.now()}.jpg`;
    link.href = canvas.toDataURL("image/jpeg", 0.95);
    link.click();
}

async function saveThumbnailToServer() {
    const canvas = document.getElementById("thumbnail-canvas");
    if (!canvas) return;

    const titleSlug = (state.thumb.title || "thumbnail").toLowerCase().replace(/[^a-z0-9]/g, "_").substring(0, 30);
    const filename = `thumbnail_${titleSlug}_${Date.now()}.jpg`;
    const dataUrl = canvas.toDataURL("image/jpeg", 0.95);

    try {
        const res = await fetch("/api/thumbnail/save", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({
                image_data: dataUrl,
                filename: filename
            })
        });

        const data = await res.json();
        if (!res.ok) throw new Error(data.detail || "Gagal menyimpan thumbnail.");

        alert(`Thumbnail berhasil disimpan!\n\nNama file: ${data.filename}\nLokasi: Folder "outputs" komputer Anda.`);
    } catch (e) {
        alert(e.message);
    }
}



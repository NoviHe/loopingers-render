import sys
import os
import time
import socket
import threading
import subprocess
import uvicorn

# Ensure core and parent directory are in sys.path
BASE_DIR = os.path.abspath(os.path.dirname(__file__))
if BASE_DIR not in sys.path:
    sys.path.insert(0, BASE_DIR)

from app import app


def find_free_port(start_port=8000, max_tries=50):
    """Finds an open localhost port starting from start_port."""
    for port in range(start_port, start_port + max_tries):
        try:
            with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
                s.bind(("127.0.0.1", port))
                return port
        except OSError:
            continue
    return start_port


def wait_for_server(port, timeout=20.0):
    """Waits until the local FastAPI server responds to TCP connections."""
    start_time = time.time()
    while time.time() - start_time < timeout:
        try:
            with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
                s.settimeout(1.0)
                if s.connect_ex(("127.0.0.1", port)) == 0:
                    return True
        except Exception:
            pass
        time.sleep(0.2)
    return False


def start_uvicorn(port):
    """Worker function to run uvicorn in a daemon thread."""
    config = uvicorn.Config(
        app=app,
        host="127.0.0.1",
        port=port,
        log_level="error",
        access_log=False,
        loop="asyncio"
    )
    server = uvicorn.Server(config)
    server.run()


def launch_edge_app_mode(url):
    """Launches Microsoft Edge or Google Chrome in standalone desktop window mode."""
    possible_browsers = [
        os.path.expandvars(r"%ProgramFiles(x86)%\Microsoft\Edge\Application\msedge.exe"),
        os.path.expandvars(r"%ProgramFiles%\Microsoft\Edge\Application\msedge.exe"),
        os.path.expandvars(r"%ProgramFiles%\Google\Chrome\Application\chrome.exe"),
        os.path.expandvars(r"%LocalAppData%\Microsoft\Edge\Application\msedge.exe"),
        os.path.expandvars(r"%LocalAppData%\Google\Chrome\Application\chrome.exe")
    ]
    browser_exe = next((p for p in possible_browsers if os.path.exists(p)), None)

    if browser_exe:
        cmd = [
            browser_exe,
            f"--app={url}",
            "--window-size=1380,880",
            "--window-position=100,50"
        ]
        proc = subprocess.Popen(cmd)
        proc.wait()
        return True
    return False


def main():
    print("=" * 60)
    print(" LoopKaraoke Studio - Desktop Edition")
    print("=" * 60)
    print("Memulai backend server...")

    port = find_free_port(8000)
    server_thread = threading.Thread(target=start_uvicorn, args=(port,), daemon=True)
    server_thread.start()

    if not wait_for_server(port):
        print("Error: Gagal memulai backend server pada port", port)
        sys.exit(1)

    app_url = f"http://127.0.0.1:{port}"
    print(f"Server aktif di: {app_url}")
    print("Membuka jendela aplikasi desktop...")

    # 1. Attempt native window with pywebview (Edge WebView2 modern Chromium engine)
    opened_native = False
    try:
        import webview
        window = webview.create_window(
            title="LoopKaraoke Studio - Desktop Edition",
            url=app_url,
            width=1380,
            height=880,
            min_size=(1024, 680),
            background_color="#0F172A",
            text_select=True,
            confirm_close=False
        )
        # gui='edgechromium' utilizes Windows 10/11 native WebView2
        webview.start(gui="edgechromium")
        opened_native = True
    except Exception as e:
        print(f"Catatan: pywebview tidak dapat dibuka ({e}). Beralih ke Edge App Mode...")

    # 2. Fallback to Microsoft Edge standalone App Mode
    if not opened_native:
        success = launch_edge_app_mode(app_url)
        if not success:
            import webbrowser
            print("Membuka di peramban default...")
            webbrowser.open(app_url)
            # Keep process alive until user cancels in console
            try:
                while True:
                    time.sleep(1)
            except KeyboardInterrupt:
                pass

    print("Menutup LoopKaraoke Studio...")
    sys.exit(0)


if __name__ == "__main__":
    main()

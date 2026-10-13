import os
import sys
import time
import webbrowser
import threading
import uvicorn

def open_browser():
    time.sleep(1.5)
    webbrowser.open("http://127.0.0.1:8000")

if __name__ == "__main__":
    print("=" * 60)
    print("      LOOPKARAOKE STUDIO - Aplikasi Pembuat Video Karaoke")
    print("      Looping Video + Gabung Lagu + Auto Karaoke AI")
    print("=" * 60)
    print("\n[+] Memulai server aplikasi di http://127.0.0.1:8000 ...")
    print("[+] Browser web akan terbuka secara otomatis.\n")

    threading.Thread(target=open_browser, daemon=True).start()

    uvicorn.run("app:app", host="127.0.0.1", port=8000, reload=False)

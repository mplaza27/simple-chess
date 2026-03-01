"""
Gzip-enabled static file server for the chess WASM app.

Python's built-in http.server doesn't compress responses. This wrapper adds
gzip encoding so browsers download ~200 KB instead of 1.2 MB for the WASM file.

Usage:
    python3 serve.py
Then open: http://<server-ip>:8080
"""

import gzip
import os
from http.server import HTTPServer, SimpleHTTPRequestHandler

DIST = os.path.join(os.path.dirname(__file__), "dist")

COMPRESSIBLE = {
    ".wasm", ".js", ".html", ".css", ".json", ".svg", ".txt"
}

class GzipHandler(SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=DIST, **kwargs)

    def send_response_with_gzip(self, path):
        """Send a file response with gzip encoding if the client accepts it."""
        accepts_gzip = "gzip" in self.headers.get("Accept-Encoding", "")
        _, ext = os.path.splitext(path)
        should_compress = accepts_gzip and ext in COMPRESSIBLE

        try:
            with open(path, "rb") as f:
                data = f.read()
        except OSError:
            self.send_error(404)
            return

        if should_compress:
            data = gzip.compress(data, compresslevel=6)

        # Guess MIME type
        mime = {
            ".html": "text/html",
            ".js":   "application/javascript",
            ".wasm": "application/wasm",
            ".css":  "text/css",
            ".json": "application/json",
        }.get(ext, "application/octet-stream")

        self.send_response(200)
        self.send_header("Content-Type", mime)
        self.send_header("Content-Length", str(len(data)))
        if should_compress:
            self.send_header("Content-Encoding", "gzip")
        # Allow browsers to cache assets (they have content-hashed names)
        if ext in {".wasm", ".js"}:
            self.send_header("Cache-Control", "public, max-age=3600")
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        # Strip query string
        path = self.path.split("?")[0].split("#")[0]
        # Map URL path to filesystem path
        fs_path = os.path.join(DIST, path.lstrip("/"))

        if os.path.isdir(fs_path):
            fs_path = os.path.join(fs_path, "index.html")

        if os.path.isfile(fs_path):
            self.send_response_with_gzip(fs_path)
        else:
            # SPA fallback: always serve index.html
            self.send_response_with_gzip(os.path.join(DIST, "index.html"))

    def log_message(self, fmt, *args):
        # Clean up log output
        print(f"  {self.address_string()} {fmt % args}")


if __name__ == "__main__":
    host, port = "0.0.0.0", 8080
    server = HTTPServer((host, port), GzipHandler)
    print(f"Serving dist/ with gzip on http://{host}:{port}")
    print(f"Open: http://10.0.0.7:{port}")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\nStopped.")

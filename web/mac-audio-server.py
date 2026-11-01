#!/usr/bin/env python3
"""Local-only web visualiser server and opt-in macOS system-audio bridge.

Audio is streamed directly from the signed helper to one browser listener; it
is never saved, logged, or sent to an external server. Stop the server with
Ctrl+C. A normal browser without this server can still play files or share audio.
"""

import argparse
import json
import mimetypes
import os
import select
import socket
import subprocess
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parent.parent
HELPER = ROOT / 'outputs/amaraMusicVisualiser.app/Contents/Helpers/amaraAudioCapture'
RATE = 48_000
MAX_CHUNK = 16_384
START_TIMEOUT = 12


class AudioServer(ThreadingHTTPServer):
    daemon_threads = True
    allow_reuse_address = False

    def __init__(self, port, helper=HELPER):
        super().__init__(('127.0.0.1', port), Handler)
        self.helper = Path(helper)
        self.capture_lock = threading.Lock()
        self.capture = None

    @property
    def authority(self):
        return f'127.0.0.1:{self.server_address[1]}'

    def stop_capture(self):
        with self.capture_lock:
            child = self.capture
            if child is not None and child.poll() is None:
                child.terminate()
        if child is not None:
            try:
                child.wait(timeout=3)
            except subprocess.TimeoutExpired:
                child.kill()
                child.wait()

    def server_close(self):
        self.stop_capture()
        super().server_close()


class Handler(BaseHTTPRequestHandler):
    protocol_version = 'HTTP/1.1'

    def log_message(self, format, *args):
        # Do not record requests or audio-related information.
        pass

    def json_response(self, status, value):
        body = json.dumps(value, separators=(',', ':')).encode()
        self.send_response(status)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(body)))
        self.send_header('Cache-Control', 'no-store')
        self.end_headers()
        self.wfile.write(body)

    def authorised_post(self):
        return (self.headers.get('Host') == self.server.authority and
                self.headers.get('Origin') == f'http://{self.server.authority}' and
                self.headers.get('Content-Length', '0') == '0' and
                self.headers.get('Transfer-Encoding') is None)

    def do_POST(self):
        if not self.authorised_post():
            self.close_connection = True
            self.json_response(403, {'error': 'Request origin is not allowed.'})
            return
        path = urlsplit(self.path).path
        if path in ('/api/system-audio/status', '/api/mac-audio/status'):
            with self.server.capture_lock:
                active = self.server.capture is not None and self.server.capture.poll() is None
            self.json_response(200, {'available': self.server.helper.is_file(),
                                     'active': active, 'sampleRate': RATE,
                                     'channels': 1, 'format': 'f32le'})
        elif path in ('/api/system-audio/stop', '/api/mac-audio/stop'):
            self.server.stop_capture()
            self.json_response(200, {'stopped': True})
        elif path in ('/api/system-audio/stream', '/api/mac-audio/stream'):
            self.stream_audio()
        else:
            self.json_response(404, {'error': 'Unknown endpoint.'})

    def stream_audio(self):
        self.close_connection = True
        with self.server.capture_lock:
            if self.server.capture is not None:
                self.json_response(409, {'error': 'Computer audio is already in use by this server.'})
                return
            if not self.server.helper.is_file():
                self.json_response(503, {'error': 'The signed Mac audio helper is not installed.'})
                return
            try:
                child = subprocess.Popen([str(self.server.helper)], stdout=subprocess.PIPE,
                                         stderr=subprocess.PIPE, bufsize=0)
            except OSError:
                self.json_response(503, {'error': 'The Mac audio helper could not be started.'})
                return
            self.server.capture = child
        started = False
        responded = False
        pipes = [child.stdout, child.stderr]
        try:
            self.connection.settimeout(2)
            deadline = time.monotonic() + START_TIMEOUT
            while child.poll() is None:
                if not started and time.monotonic() >= deadline:
                    self.json_response(504, {'error': 'Computer audio did not start. Check macOS Audio Capture permission.'})
                    responded = True
                    return
                ready, _, _ = select.select(pipes, [], [], 0.25)
                for pipe in ready:
                    data = os.read(pipe.fileno(), MAX_CHUNK)
                    if not data:
                        if pipe is child.stdout:
                            return
                        pipes.remove(pipe)
                        continue
                    if pipe is child.stderr:
                        continue
                    if not started:
                        self.send_response(200)
                        self.send_header('Content-Type', 'application/octet-stream')
                        self.send_header('Cache-Control', 'no-store')
                        self.send_header('Connection', 'close')
                        self.send_header('Transfer-Encoding', 'chunked')
                        self.end_headers()
                        started = True
                    self.wfile.write(f'{len(data):x}\r\n'.encode() + data + b'\r\n')
                    self.wfile.flush()
        except (BrokenPipeError, ConnectionResetError, socket.timeout):
            pass
        finally:
            if child.poll() is None:
                child.terminate()
            try:
                child.wait(timeout=3)
            except subprocess.TimeoutExpired:
                child.kill()
                child.wait()
            child.stdout.close()
            child.stderr.close()
            with self.server.capture_lock:
                if self.server.capture is child:
                    self.server.capture = None
            if not started and not responded:
                # Avoid putting helper output (which may contain device details)
                # in logs or the browser; offer a safe, actionable status.
                try:
                    self.json_response(503, {'error': 'Computer audio stopped before starting. Check macOS Audio Capture permission.'})
                except (BrokenPipeError, ConnectionResetError, socket.timeout):
                    pass
            elif started:
                try:
                    self.wfile.write(b'0\r\n\r\n')
                    self.wfile.flush()
                except (BrokenPipeError, ConnectionResetError, socket.timeout):
                    pass

    def do_GET(self):
        self.serve_asset(False)

    def do_HEAD(self):
        self.serve_asset(True)

    def serve_asset(self, head):
        if self.headers.get('Host') != self.server.authority:
            self.send_error(403)
            return
        url_path = unquote(urlsplit(self.path).path)
        if url_path == '/':
            url_path = '/web/'
        if url_path == '/web/':
            url_path = '/web/index.html'
        asset = (ROOT / url_path.lstrip('/')).resolve()
        web_root = (ROOT / 'web').resolve()
        src_root = (ROOT / 'src').resolve()
        allowed = asset.is_relative_to(web_root) or (asset.is_relative_to(src_root) and asset.suffix == '.wgsl')
        if not allowed or not asset.is_file() or asset.suffix == '.py':
            self.send_error(404)
            return
        try:
            with asset.open('rb') as stream:
                self.send_response(200)
                self.send_header('Content-Type', mimetypes.guess_type(asset.name)[0] or 'application/octet-stream')
                self.send_header('Content-Length', str(asset.stat().st_size))
                self.send_header('Cache-Control', 'no-store')
                self.send_header('X-Content-Type-Options', 'nosniff')
                self.send_header('X-Frame-Options', 'DENY')
                self.send_header('Content-Security-Policy', "frame-ancestors 'none'")
                self.end_headers()
                if not head:
                    while True:
                        block = stream.read(65536)
                        if not block:
                            break
                        self.wfile.write(block)
        except (OSError, BrokenPipeError, ConnectionResetError):
            self.close_connection = True


def main():
    parser = argparse.ArgumentParser(description='Run the local amaraMusicVisualiser web audio bridge')
    parser.add_argument('--port', type=int, default=8768, help='Loopback port (default: 8768)')
    args = parser.parse_args()
    if not 1 <= args.port <= 65535:
        parser.error('port must be between 1 and 65535')
    try:
        server = AudioServer(args.port)
    except OSError as error:
        print(f'Could not start local visualiser: {error}', file=sys.stderr)
        return 1
    print(f'amaraMusicVisualiser: http://127.0.0.1:{args.port}/web/', flush=True)
    try:
        server.serve_forever(poll_interval=0.25)
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()
    return 0


if __name__ == '__main__':
    raise SystemExit(main())

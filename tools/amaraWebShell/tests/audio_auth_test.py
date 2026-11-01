"""Mac-only integration test: local audio authorization with synthetic PCM."""
import http.client
import json
import os
from pathlib import Path
import plistlib
import struct
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[3]

@unittest.skipUnless(sys.platform == 'darwin', 'requires macOS Swift and WebKit')
class AudioAuthTest(unittest.TestCase):
    def test_forgery_rejected_and_authorized_stream_works(self):
        with tempfile.TemporaryDirectory(prefix='amara-audio-auth-') as tmp:
            root = Path(tmp).resolve()
            bundle = root / 'synthetic.app'
            contents = bundle / 'Contents'
            runtime = contents / 'Resources/runtime'
            (runtime/'web').mkdir(parents=True)
            (runtime/'src').mkdir()
            (contents/'Helpers').mkdir()
            (contents/'Info.plist').write_bytes(plistlib.dumps({'CFBundleIdentifier':'test.amara.audio-auth', 'CFBundleName':'Synthetic audio test', 'CFBundlePackageType':'APPL'}))
            (runtime/'web/index.html').write_text('<!doctype html><title>synthetic</title>')
            (runtime/'src/shader.wgsl').write_text('// synthetic')
            marker = root/'helper-started'
            helper = contents/'Helpers/amaraAudioCapture'
            helper.write_text('#!/usr/bin/python3\nimport pathlib,sys,struct,time\npathlib.Path('+repr(str(marker))+').write_text("synthetic")\nwhile True:\n sys.stdout.buffer.write(struct.pack("<f",0.25)*1024)\n sys.stdout.buffer.flush()\n time.sleep(0.02)\n')
            helper.chmod(0o755)
            source = (ROOT/'tools/amaraWebShell/main.swift').read_text()
            original = 'stableOrigin = try StableOrigin(directory: support.appendingPathComponent("com.amara.musicvisualiser.web", isDirectory: true))'
            self.assertIn(original, source)
            source = source.replace(original, 'stableOrigin = try StableOrigin(directory: URL(fileURLWithPath: '+json.dumps(str(root/'settings'))+'))', 1)
            source = source[:source.index('@main\nprivate struct AmaraWebShell')] + '''@main private struct TestServer {
 static func main() {
  do { let bundle=Bundle(url:URL(fileURLWithPath:TEST_BUNDLE))!; let server=try LoopbackServer(bundle:bundle);try server.start();print("PORT=\\(server.port) TOKEN=\\(server.sessionToken)");fflush(stdout);sleep(30);server.stop() }
  catch {print("TEST_START_ERROR");exit(2)}
 }
}
'''.replace('TEST_BUNDLE',json.dumps(str(bundle)))
            test_source=root/'TestServer.swift';test_source.write_text(source)
            exe=root/'test-server'
            subprocess.run(['/usr/bin/xcrun','swiftc','-parse-as-library',str(ROOT/'tools/amaraWebShell/StableOrigin.swift'),str(test_source),'-o',str(exe),'-framework','AppKit','-framework','WebKit','-framework','Foundation','-framework','Security'],check=True,capture_output=True,timeout=90)
            proc=subprocess.Popen([str(exe)],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
            try:
                fields=dict(item.split('=',1) for item in proc.stdout.readline().strip().split())
                port=int(fields['PORT']);token=fields['TOKEN'];self.assertEqual(len(token),64)
                def post(path, credential=None):
                    c=http.client.HTTPConnection('127.0.0.1',port,timeout=4)
                    h={'Origin':f'http://127.0.0.1:{port}','Content-Length':'0'}
                    if credential is not None:h['X-Amara-Audio-Token']=credential
                    c.request('POST',path,body='',headers=h)
                    return c,c.getresponse()
                for path in ('/api/system-audio/status','/api/system-audio/stream','/api/system-audio/stop','/api/mac-audio/status'):
                    for bad in (None,'wrong'):
                        c,r=post(path,bad);self.assertEqual(r.status,403);r.read();c.close()
                self.assertFalse(marker.exists())
                c,r=post('/api/system-audio/status',token);self.assertEqual(r.status,200);self.assertFalse(json.loads(r.read())['active']);c.close()
                stream,r=post('/api/system-audio/stream',token);self.assertEqual(r.status,200)
                data=r.read(512);self.assertEqual(struct.unpack('<f',data[:4])[0],0.25)
                c,stop=post('/api/system-audio/stop');self.assertEqual(stop.status,403);stop.read();c.close()
                c,status=post('/api/system-audio/status',token);self.assertTrue(json.loads(status.read())['active']);c.close()
                c,stop=post('/api/system-audio/stop',token);self.assertEqual(stop.status,200);stop.read();c.close()
                r.close();stream.close()
            finally:
                if proc.poll() is None:
                    proc.terminate()
                proc.wait(timeout=5)
                proc.stdout.close()
                proc.stderr.close()

if __name__ == '__main__':unittest.main()

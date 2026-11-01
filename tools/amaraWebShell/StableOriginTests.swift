import Foundation
import Darwin
@main struct Test {
 static func check(_ condition: @autoclosure () -> Bool, _ label: String) { if !condition() { print("FAIL",label); exit(1) }; print("PASS",label) }
 static func main() throws {
  let root=URL(fileURLWithPath:NSTemporaryDirectory()).appendingPathComponent("amara-origin-test-"+UUID().uuidString)
  try FileManager.default.createDirectory(at:root,withIntermediateDirectories:true,attributes:[.posixPermissions:0o700])
  let dir=root.appendingPathComponent("private"); var first:StableOrigin? = try StableOrigin(directory:dir)
  let fresh = try first!.loadPort(); check(fresh == nil,"fresh store")
  do { _ = try StableOrigin(directory:dir); check(false,"exclusive lock") } catch { print("PASS exclusive lock") }
  try first!.saveInitialPort(43210); first=nil
  let second=try StableOrigin(directory:dir); let restored = try second.loadPort(); check(restored == 43210,"relaunch reads same port")
  do { try second.saveInitialPort(43211); check(false,"no overwrite") } catch { print("PASS no overwrite") }
  let interrupted=root.appendingPathComponent("interrupted"); let recovery=try StableOrigin(directory:interrupted)
  try "12".write(to:interrupted.appendingPathComponent(".origin-interrupted.tmp"),atomically:false,encoding:.utf8)
  let before = try recovery.loadPort(); check(before == nil,"interrupted temporary file is ignored")
  try recovery.saveInitialPort(43212); let recovered = try recovery.loadPort(); check(recovered == 43212,"save after interrupted temporary file")
  let symlinkFile=root.appendingPathComponent("symlink-file"); let noFollow=try StableOrigin(directory:symlinkFile)
  try FileManager.default.createSymbolicLink(at:symlinkFile.appendingPathComponent("origin.port"),withDestinationURL:dir.appendingPathComponent("origin.port"))
  do { _ = try noFollow.loadPort(); check(false,"reject symlink port file") } catch { print("PASS reject symlink port file") }
  let other=root.appendingPathComponent("corrupt"); let bad=try StableOrigin(directory:other)
  try "not a port".write(to:other.appendingPathComponent("origin.port"),atomically:false,encoding:.utf8); chmod(other.appendingPathComponent("origin.port").path,0o600)
  do { _ = try bad.loadPort(); check(false,"reject corrupt") } catch { print("PASS reject corrupt") }
  let insecure=root.appendingPathComponent("insecure"); try FileManager.default.createDirectory(at:insecure,withIntermediateDirectories:false); chmod(insecure.path,0o755)
  do { _ = try StableOrigin(directory:insecure); check(false,"reject public dir") } catch { print("PASS reject public dir") }
  let link=root.appendingPathComponent("link"); try FileManager.default.createSymbolicLink(at:link,withDestinationURL:dir)
  do { _ = try StableOrigin(directory:link); check(false,"reject symlink") } catch { print("PASS reject symlink") }
  let a = socket(AF_INET, SOCK_STREAM, 0); let b = socket(AF_INET, SOCK_STREAM, 0)
  var reuse:Int32 = 1; _ = setsockopt(a,SOL_SOCKET,SO_REUSEADDR,&reuse,socklen_t(MemoryLayout<Int32>.size)); _ = setsockopt(b,SOL_SOCKET,SO_REUSEADDR,&reuse,socklen_t(MemoryLayout<Int32>.size))
  var addr = sockaddr_in(); addr.sin_len = UInt8(MemoryLayout<sockaddr_in>.size); addr.sin_family = sa_family_t(AF_INET); addr.sin_addr = in_addr(s_addr: inet_addr("127.0.0.1")); addr.sin_port = 0
  let firstBind = withUnsafePointer(to: &addr) { $0.withMemoryRebound(to:sockaddr.self,capacity:1) { bind(a,$0,socklen_t(MemoryLayout<sockaddr_in>.size)) } }
  check(firstBind == 0 && listen(a,1) == 0,"foreign listener starts")
  var len=socklen_t(MemoryLayout<sockaddr_in>.size); _ = withUnsafeMutablePointer(to:&addr) { $0.withMemoryRebound(to:sockaddr.self,capacity:1) { getsockname(a,$0,&len) } }
  let occupiedBind = withUnsafePointer(to:&addr) { $0.withMemoryRebound(to:sockaddr.self,capacity:1) { bind(b,$0,socklen_t(MemoryLayout<sockaddr_in>.size)) } }
  check(occupiedBind != 0 && errno == EADDRINUSE,"occupied port rejected")
  close(a); close(b)
  print("isolated path",root.path)
 }
}

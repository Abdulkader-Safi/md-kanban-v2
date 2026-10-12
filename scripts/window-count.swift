// Prints how many on-screen windows a process has. Used by bench.sh to time a cold start.
// Reads the window list only, so it needs no accessibility permission.
import CoreGraphics
import Foundation

let pid = Int(CommandLine.arguments[1])!
let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as! [[String: Any]]
let count = list.filter { window in
    let bounds = window[kCGWindowBounds as String] as? [String: Any]
    return (window[kCGWindowOwnerPID as String] as? Int) == pid
        && (window[kCGWindowLayer as String] as? Int) == 0
        && (bounds?["Height"] as? Double ?? 0) > 200
}.count
print(count)

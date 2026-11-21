// Renders the Zotebook app icon, in the style of the other Klectr Z apps (Zwisper):
// a deep Catppuccin Mocha squircle with a glowing lavender → mauve → blue mark.
// The mark is Zotebook's three stacked pages; the front page carries the gradient
// and a few lines of writing.
//
//   swiftc -O -o /tmp/render-icon packaging/icon/render.swift
//   /tmp/render-icon <out.png> [size] [mac|full]
//
// `mac` (the default) draws on Apple's macOS icon grid: an 824 px body in a 1024 px
// canvas, with the margin left for the drop shadow. `full` fills the canvas, for the
// Windows and Linux icons. Every size is drawn natively: at 32 px and below the mark
// grows, loses the back page, the glow and the writing, so it stays legible.
// make-icons.sh builds every icon file from this script.
import AppKit

let out = URL(fileURLWithPath: CommandLine.arguments[1])
let px = CommandLine.arguments.count > 2 ? Int(CommandLine.arguments[2])! : 1024
let mac = CommandLine.arguments.count > 3 ? CommandLine.arguments[3] != "full" : true
let small = px <= 32
let S: CGFloat = 1024                       // design units; scaled to `px` below
func hex(_ v: UInt32, _ a: CGFloat = 1) -> NSColor {
    NSColor(srgbRed: CGFloat((v >> 16) & 0xff) / 255, green: CGFloat((v >> 8) & 0xff) / 255, blue: CGFloat(v & 0xff) / 255, alpha: a)
}
// Catppuccin Mocha
let crust = hex(0x11111b), surface0 = hex(0x313244)
let mauve = hex(0xcba6f7), blue = hex(0x89b4fa), lavender = hex(0xb4befe)

/// A continuous-corner ("squircle") rounded rect, closer to Apple's icon shape than
/// a plain circular-arc rounded rect.
func squircle(_ r: CGRect, radius: CGFloat) -> NSBezierPath {
    let p = NSBezierPath()
    let k: CGFloat = 1.28   // how far the curve reaches along each edge, in radii
    let c: CGFloat = 0.55   // control-point pull
    let (x0, y0, x1, y1) = (r.minX, r.minY, r.maxX, r.maxY)
    let e = radius * k
    p.move(to: NSPoint(x: x0 + e, y: y0))
    p.line(to: NSPoint(x: x1 - e, y: y0))
    p.curve(to: NSPoint(x: x1, y: y0 + e), controlPoint1: NSPoint(x: x1 - e * (1 - c), y: y0), controlPoint2: NSPoint(x: x1, y: y0 + e * (1 - c)))
    p.line(to: NSPoint(x: x1, y: y1 - e))
    p.curve(to: NSPoint(x: x1 - e, y: y1), controlPoint1: NSPoint(x: x1, y: y1 - e * (1 - c)), controlPoint2: NSPoint(x: x1 - e * (1 - c), y: y1))
    p.line(to: NSPoint(x: x0 + e, y: y1))
    p.curve(to: NSPoint(x: x0, y: y1 - e), controlPoint1: NSPoint(x: x0 + e * (1 - c), y: y1), controlPoint2: NSPoint(x: x0, y: y1 - e * (1 - c)))
    p.line(to: NSPoint(x: x0, y: y0 + e))
    p.curve(to: NSPoint(x: x0 + e, y: y0), controlPoint1: NSPoint(x: x0, y: y0 + e * (1 - c)), controlPoint2: NSPoint(x: x0 + e * (1 - c), y: y0))
    p.close()
    return p
}

let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: px, pixelsHigh: px, bitsPerSample: 8,
                           samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB,
                           bytesPerRow: 0, bitsPerPixel: 0)!
NSGraphicsContext.saveGraphicsState()
let ctx = NSGraphicsContext(bitmapImageRep: rep)!
NSGraphicsContext.current = ctx
let cg = ctx.cgContext
cg.scaleBy(x: CGFloat(px) / S, y: CGFloat(px) / S)

let body = mac ? CGRect(x: 100, y: 100, width: 824, height: 824) : CGRect(x: 24, y: 24, width: 976, height: 976)
let radius = body.width * 185 / 824
let shape = squircle(body, radius: radius)

// Drop shadow in the macOS margin. Skipped when small, where it only reads as a halo.
cg.saveGState()
if mac && !small {
    cg.setShadow(offset: CGSize(width: 0, height: -12), blur: 28, color: NSColor.black.withAlphaComponent(0.38).cgColor)
}
crust.setFill(); shape.fill()
cg.restoreGState()

// Body: deep Mocha gradient, lighter at the top, with a soft glow behind the mark.
cg.saveGState()
shape.addClip()
NSGradient(colors: [hex(0x2b2b42), hex(0x1e1e2e), crust], atLocations: [0, 0.45, 1], colorSpace: .sRGB)!
    .draw(in: body, angle: -90)
if !small {
    NSGradient(colors: [mauve.withAlphaComponent(0.26), blue.withAlphaComponent(0.09), mauve.withAlphaComponent(0)],
               atLocations: [0, 0.45, 1], colorSpace: .sRGB)!
        .draw(fromCenter: NSPoint(x: S / 2 - 30, y: S / 2 - 40), radius: 0,
              toCenter: NSPoint(x: S / 2 - 30, y: S / 2 - 40), radius: body.width * 0.52, options: [])
}
cg.restoreGState()

// Edge: faint light rim inside, hairline all round.
cg.saveGState()
shape.addClip()
let rim = squircle(body.insetBy(dx: 2, dy: 2), radius: radius - 2)
rim.lineWidth = 4
NSColor.white.withAlphaComponent(0.10).setStroke(); rim.stroke()
cg.restoreGState()
surface0.withAlphaComponent(0.9).setStroke()
shape.lineWidth = 2; shape.stroke()

// The mark: the brand's three pages (46 × 60 units, 6-unit corners, each 11 across and
// 10 down from the one behind) on a 100-unit grid, scaled into the middle of the body.
// When small, only the front page and the one behind it.
let unit = (small ? 0.84 : 0.62) * body.width / 80
func page(_ x: CGFloat, _ y: CGFloat) -> NSRect {
    // Brand units run downward from the top left; AppKit's run upward.
    NSRect(x: S / 2 + (x - 50) * unit, y: S / 2 - (y + 60 - 50) * unit, width: 46 * unit, height: 60 * unit)
}
let corner = 6 * unit
let back = page(38, 10), middle = small ? page(33, 15) : page(27, 20), front = small ? page(22, 25) : page(16, 30)

// Back pages: muted Mocha tones, lighter at the top, with a light hairline.
for (r, top, bottom) in (small ? [(middle, hex(0x6c7086), hex(0x585b70))]
                               : [(back, hex(0x45475a), hex(0x313244)), (middle, hex(0x6c7086), hex(0x4f5168))]) {
    let p = NSBezierPath(roundedRect: r, xRadius: corner, yRadius: corner)
    cg.saveGState()
    if !small { cg.setShadow(offset: CGSize(width: 0, height: -6), blur: 18, color: NSColor.black.withAlphaComponent(0.35).cgColor) }
    top.setFill(); p.fill()
    cg.restoreGState()
    cg.saveGState()
    p.addClip()
    NSGradient(colors: [top, bottom], atLocations: [0, 1], colorSpace: .sRGB)!.draw(in: r, angle: -90)
    cg.restoreGState()
    if !small {
        let edge = NSBezierPath(roundedRect: r.insetBy(dx: 1.5, dy: 1.5), xRadius: corner - 1.5, yRadius: corner - 1.5)
        edge.lineWidth = 3
        NSColor.white.withAlphaComponent(0.10).setStroke(); edge.stroke()
    }
}

// Front page: the glow, then the lavender → mauve → blue gradient.
let frontPath = NSBezierPath(roundedRect: front, xRadius: corner, yRadius: corner)
if !small {
    cg.saveGState()
    cg.setShadow(offset: .zero, blur: 64, color: mauve.withAlphaComponent(0.55).cgColor)
    mauve.setFill(); frontPath.fill()
    cg.restoreGState()
}
cg.saveGState()
frontPath.addClip()
NSGradient(colors: [lavender, mauve, blue], atLocations: [0, 0.5, 1], colorSpace: .sRGB)!.draw(in: front, angle: -90)
cg.restoreGState()

// Writing on the front page: a heading and two lines of text, in the page's own ink.
if !small {
    let ink = hex(0x1e1e2e, 0.30)
    let left = front.minX + 8 * unit
    for (dy, w, h) in [(CGFloat(10), CGFloat(20), CGFloat(4.2)), (21, 30, 3), (29, 30, 3), (37, 19, 3)] {
        let r = NSRect(x: left, y: front.maxY - (dy + h) * unit, width: w * unit, height: h * unit)
        ink.setFill(); NSBezierPath(roundedRect: r, xRadius: h * unit / 2, yRadius: h * unit / 2).fill()
    }
}

NSGraphicsContext.restoreGraphicsState()
try! rep.representation(using: .png, properties: [:])!.write(to: out)
print("wrote", out.path)

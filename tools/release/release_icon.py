"""Generate the original geokernel triangle icon without tracking binary assets."""

import math
import struct
import zlib


def png(size=256):
    scale = 256 / size
    vertices = ((64, 179), (122, 64), (194, 179))
    raw = bytearray()
    for row in range(size):
        y = (row + 0.5) * scale - 0.5
        raw.append(0)
        for column in range(size):
            x = (column + 0.5) * scale - 0.5
            radius = math.hypot(max(32 - x, x - 223, 0), max(32 - y, y - 223, 0))
            if radius > 29:
                raw.extend((0, 0, 0, 0))
                continue
            color = (21, 43, 58, 255)
            if x % 16 < 1 or y % 16 < 1:
                color = (39, 65, 79, 255)
            for (ax, ay), (bx, by) in zip(vertices, vertices[1:] + vertices[:1]):
                factor = ((x - ax) * (bx - ax) + (y - ay) * (by - ay)) / (
                    (bx - ax) ** 2 + (by - ay) ** 2
                )
                if 0 <= factor <= 1 and math.hypot(
                    x - ax - factor * (bx - ax), y - ay - factor * (by - ay)
                ) <= 4:
                    color = (92, 230, 194, 255)
            if any((x - vx) ** 2 + (y - vy) ** 2 <= 9**2 for vx, vy in vertices):
                color = (239, 254, 245, 255)
            raw.extend(color)

    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))

    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(raw))
        + chunk(b"IEND", b"")
    )


def write_icon(path):
    image = png()
    if path.suffix == ".icns":
        image = b"icns" + struct.pack(">I", 16 + len(image)) + b"ic08" + struct.pack(
            ">I", 8 + len(image)
        ) + image
    elif path.suffix == ".ico":
        sizes = (16, 32, 48, 64, 256)
        header = struct.pack("<HHH", 0, 1, len(sizes))
        offset = len(header) + 16 * len(sizes)
        entries, frames = [], []
        for size in sizes:
            frame = image if size == 256 else png(size)
            entries.append(struct.pack("<BBBBHHII", size % 256, size % 256, 0, 0, 1, 32, len(frame), offset))
            frames.append(frame)
            offset += len(frame)
        image = header + b"".join(entries + frames)
    path.write_bytes(image)

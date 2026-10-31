#!/usr/bin/env python3
"""minecraft-join-bot.py PACKETS_JSON [PORT] [SECONDS]: joins a Minecraft server in offline mode as player
"bot", as a client does: login (with compression), configuration (known packs echoed back), then
play: answers keep-alives and the first teleport, acknowledges chunk batches, says hello through
`/say`, stays SECONDS (default 15), and leaves. Prints what it saw; exits 1 if it was kicked or
never reached play. Packet numbers come from the server's own report (packets.json of
`net.minecraft.data.Main --reports`); the survey's join step runs it (docs/minecraft-server.md)."""
import json
import socket
import struct
import sys
import time
import uuid
import zlib

ids = json.load(open(sys.argv[1]))
PORT = int(sys.argv[2]) if len(sys.argv) > 2 else 25599
SECONDS = float(sys.argv[3]) if len(sys.argv) > 3 else 15


def pid(phase, side, name):
    return ids[phase][side]["minecraft:" + name]["protocol_id"]


def names(phase, side):
    return {v["protocol_id"]: k.removeprefix("minecraft:") for k, v in ids[phase][side].items()}


def varint(n):
    out = b""
    n &= 0xFFFFFFFF
    while True:
        b = n & 0x7F
        n >>= 7
        out += bytes([b | (0x80 if n else 0)])
        if not n:
            return out


def string(s):
    b = s.encode()
    return varint(len(b)) + b


def take_varint(data, i=0):
    n = shift = 0
    while True:
        b = data[i]
        i += 1
        n |= (b & 0x7F) << shift
        if not b & 0x80:
            return n, i
        shift += 7


def take_string(data, i):
    n, i = take_varint(data, i)
    return data[i:i + n].decode(), i + n


class Connection:
    def __init__(self, sock):
        self.sock = sock
        self.buffer = b""
        self.threshold = -1

    def recv_exact(self, n):
        while len(self.buffer) < n:
            chunk = self.sock.recv(1 << 16)
            if not chunk:
                raise EOFError("connection closed")
            self.buffer += chunk
        out, self.buffer = self.buffer[:n], self.buffer[n:]
        return out

    def read(self):
        n = shift = 0
        while True:
            b = self.recv_exact(1)[0]
            n |= (b & 0x7F) << shift
            if not b & 0x80:
                break
            shift += 7
        data = self.recv_exact(n)
        if self.threshold >= 0:
            size, i = take_varint(data)
            data = zlib.decompress(data[i:]) if size else data[i:]
        packet, i = take_varint(data)
        return packet, data[i:]

    def send(self, packet, payload=b""):
        body = varint(packet) + payload
        if self.threshold >= 0:
            if len(body) >= self.threshold:
                body = varint(len(body)) + zlib.compress(body)
            else:
                body = varint(0) + body
        self.sock.sendall(varint(len(body)) + body)


def main():
    sock = socket.create_connection(("127.0.0.1", PORT), timeout=30)
    c = Connection(sock)
    version = 777
    c.send(pid("handshake", "serverbound", "intention"),
           varint(version) + string("127.0.0.1") + struct.pack(">H", PORT) + varint(2))
    player = uuid.uuid3(uuid.NAMESPACE_OID, "OfflinePlayer:bot")
    c.send(pid("login", "serverbound", "hello"), string("bot") + player.bytes)
    login = names("login", "clientbound")
    while True:
        packet, data = c.read()
        name = login.get(packet, hex(packet))
        if name == "login_compression":
            c.threshold, _ = take_varint(data)
            print(f"compression from {c.threshold} bytes")
        elif name == "login_finished":
            print("logged in as", take_string(data, 16)[0])
            c.send(pid("login", "serverbound", "login_acknowledged"))
            break
        elif name == "login_disconnect":
            print("kicked at login:", data.decode(errors="replace")[:300])
            return 1
        else:
            print("login packet", name)
    configuration = names("configuration", "clientbound")
    seen = {}
    while True:
        packet, data = c.read()
        name = configuration.get(packet, hex(packet))
        seen[name] = seen.get(name, 0) + 1
        if name == "select_known_packs":
            c.send(pid("configuration", "serverbound", "select_known_packs"), data)
        elif name == "keep_alive":
            c.send(pid("configuration", "serverbound", "keep_alive"), data)
        elif name == "ping":
            c.send(pid("configuration", "serverbound", "pong"), data)
        elif name == "disconnect":
            print("kicked in configuration:", data[:300])
            return 1
        elif name == "finish_configuration":
            c.send(pid("configuration", "serverbound", "finish_configuration"))
            break
    print("configuration:", ", ".join(f"{k} x{v}" for k, v in sorted(seen.items())))
    play = names("play", "clientbound")
    seen = {}
    said = teleported = False
    start = time.monotonic()
    sock.settimeout(1)
    while time.monotonic() - start < SECONDS:
        try:
            packet, data = c.read()
        except socket.timeout:
            continue
        name = play.get(packet, hex(packet))
        seen[name] = seen.get(name, 0) + 1
        if name == "keep_alive":
            c.send(pid("play", "serverbound", "keep_alive"), data)
        elif name == "ping":
            c.send(pid("play", "serverbound", "pong"), data)
        elif name == "player_position":
            # The id, then the position, the movement and the rotation; the answer gives back
            # the id with the position and the rotation (26.3).
            teleport, i = take_varint(data)
            x, y, z = struct.unpack(">ddd", data[i:i + 24])
            yaw, pitch = struct.unpack(">ff", data[i + 48:i + 56])
            c.send(pid("play", "serverbound", "accept_teleportation"),
                   varint(teleport) + struct.pack(">dddff", x, y, z, yaw, pitch))
            if not teleported:
                teleported = True
                c.send(pid("play", "serverbound", "player_loaded"))
        elif name == "chunk_batch_finished":
            c.send(pid("play", "serverbound", "chunk_batch_received"), struct.pack(">f", 64.0))
        elif name == "disconnect":
            print("kicked in play:", data[:300])
            return 1
        if teleported and not said and seen.get("level_chunk_with_light", 0) >= 49:
            said = True
            c.send(pid("play", "serverbound", "chat_command"), string("me says hello"))
    print("play:", ", ".join(f"{k} x{v}" for k, v in sorted(seen.items(), key=lambda kv: -kv[1])[:25]))
    print(f"chunks {seen.get('level_chunk_with_light', 0)}, teleported {teleported}, said {said}")
    sock.close()
    return 0 if teleported and seen.get("level_chunk_with_light", 0) > 0 else 1


if __name__ == "__main__":
    sys.exit(main())

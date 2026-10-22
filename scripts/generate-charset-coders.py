#!/usr/bin/env python3
"""Writes the translator's TranslatedCoders.java from the JDK's sources of the standard charsets.

usage: generate-charset-coders.py JAVA_BASE_SOURCES OUTPUT

JAVA_BASE_SOURCES is the directory of java.base's sources (as for the whole-program test), and
OUTPUT is crates/translator/java/overlay/sun/nio/cs/TranslatedCoders.java. Each charset's nested
Decoder and Encoder classes are copied as they are, renamed after the charset (Utf8Decoder), with
UTF_8.updatePositions that the UTF-8 coders call.
"""
import re
import sys

src = sys.argv[1] + "/sun/nio/cs/"
out = sys.argv[2]
charsets = [("UTF_8", "Utf8", "UTF_8"), ("US_ASCII", "UsAscii", "US_ASCII"), ("ISO_8859_1", "Latin1", "ISO_8859_1"),
            ("UTF_16", "Utf16", "UTF_16"), ("UTF_16BE", "Utf16Be", "UTF_16BE"), ("UTF_16LE", "Utf16Le", "UTF_16LE")]

def block(text, start):
    i = text.index("{", start)
    depth = 0
    for j in range(i, len(text)):
        if text[j] == "{": depth += 1
        elif text[j] == "}":
            depth -= 1
            if depth == 0:
                return text[start:j + 1]
    raise Exception("unbalanced")

def line_start(text, i):
    return text.rindex("\n", 0, i) + 1

parts = []
for file, prefix, const in charsets:
    text = open(src + file + ".java").read()
    body = []
    for name in ("Decoder", "Encoder"):
        m = re.search(r"\n    private static (final )?class " + name + r"\b", text)
        b = block(text, m.start() + 1)
        b = re.sub(r"\bDecoder\b", prefix + "Decoder", b)
        b = re.sub(r"\bEncoder\b", prefix + "Encoder", b)
        body.append(b)
    parts.append((file, prefix, const, body))

utf8 = open(src + "UTF_8.java").read()
update = block(utf8, line_start(utf8, utf8.index("static final void updatePositions")))

lines = []
lines.append("""package sun.nio.cs;

import java.nio.Buffer;
import java.nio.ByteBuffer;
import java.nio.CharBuffer;
import java.nio.charset.Charset;
import java.nio.charset.CharsetDecoder;
import java.nio.charset.CharsetEncoder;
import java.nio.charset.CoderResult;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.StandardCharsets;
import java.util.Objects;

import jdk.internal.access.JavaLangAccess;
import jdk.internal.access.SharedSecrets;
import jdk.internal.util.Preconditions;
import jdk.internal.vm.annotation.IntrinsicCandidate;

/**
 * The decoders and encoders of the charsets that a translated program has, which the calls of
 * {@code newDecoder} and {@code newEncoder} on a charset make ({@code lower/charsets.rs}): a
 * charset is the runtime's value, so the JDK's classes of the standard charsets are not
 * translated, but their coders are. Each is the JDK's nested {@code Decoder} or {@code Encoder}
 * of {@code UTF_8}, {@code US_ASCII}, {@code ISO_8859_1}, {@code UTF_16}, {@code UTF_16BE} or
 * {@code UTF_16LE}, renamed after its charset, with {@code UTF_8.updatePositions} that the UTF-8
 * ones call; {@code scripts/generate-charset-coders.py} writes this file from those sources. A
 * charset without coders here (none, as the runtime has only these) is refused with an
 * {@code UnsupportedOperationException}.
 */
public final class TranslatedCoders {
    private TranslatedCoders() {}

    private static final JavaLangAccess JLA = SharedSecrets.getJavaLangAccess();

    /** {@code cs.newDecoder()}. */
    public static CharsetDecoder newDecoder(Charset cs) {""")
for file, prefix, const, body in parts:
    lines.append(f"        if (cs == StandardCharsets.{const}) {{\n            return new {prefix}Decoder(cs);\n        }}")
lines.append("""        throw new UnsupportedOperationException("a decoder of " + cs);
    }

    /** {@code cs.newEncoder()}. */
    public static CharsetEncoder newEncoder(Charset cs) {""")
for file, prefix, const, body in parts:
    lines.append(f"        if (cs == StandardCharsets.{const}) {{\n            return new {prefix}Encoder(cs);\n        }}")
lines.append("""        throw new UnsupportedOperationException("an encoder of " + cs);
    }
""")
lines.append("    " + update.strip() + "\n")
for file, prefix, const, body in parts:
    lines.append(f"    // {file}\n")
    for b in body:
        lines.append("    " + b.strip() + "\n")
lines.append("}\n")
open(out, "w").write("\n".join(lines))

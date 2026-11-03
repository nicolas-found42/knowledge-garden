# Streaming MIME probe

GMime 3.2.15 was evaluated locally using a synthetic RFC 5322 message. Its persistent-stream parser exposed a later 27-byte text part while retaining an 11,099,998-byte encoded attachment as a source-backed stream. The parser and a relinked 13-library runtime ran under a deny-by-default macOS sandbox that permitted the synthetic input, bundled runtime, and system libraries. No collection data was uploaded.

A separate prototype read a 1,720,000-byte synthetic text body in 64 KiB decoded units. It wrote and synchronized each unit and a cursor keyed by the source SHA-256, exited after two units, resumed, and reconstructed the exact body from 27 units. The prototype reparsed the message and decoded from the beginning for each unit. It is not integrated with the application, does not prove power-loss recovery, and does not establish semantic publication or source-removal fencing.

GMime is LGPL-2.1-or-later. The local Homebrew installation also links GLib, GPGME, libidn2 and other libraries. A production helper must pin and provision the complete dependency closure, include the applicable license notices, and allow only the bundled runtime in its sandbox. The container staging contract needs source ID, source version, source generation, projection kind, stable part/member locator and decoded-projection offsets. The current plain-text scheduler validates offsets against original bytes and cannot safely consume those records.

References: [GMime source](https://github.com/jstedfast/gmime), [GMime persistent-stream parser documentation](https://lazka.github.io/pgi-docs/GMime-3.0/classes/Parser.html).

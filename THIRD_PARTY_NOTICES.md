# Third party notices

The experimental desktop probe adapts the profile launch approach from [edihasaj/codex-account-switcher](https://github.com/edihasaj/codex-account-switcher), commit `5acfb58360969746fc75718d52113db5cc10c5e3`: launch the unchanged Codex app with both `CODEX_HOME` and Electron `--user-data-dir` set to separate profile directories. The upstream project is MIT licensed.

The source project reports that this launch approach works for its tested setup. Our probe still treats the result as unverified until the active account and history are checked on this Mac and Codex version.

The Rust desktop adapter adapts the PID-addressed macOS reopen event from [bartekczyz/ai-profiles](https://github.com/bartekczyz/ai-profiles/blob/8c6f685a62a8e570f57e5072c01ee070fa4ef148/apps/ai-profiles/src-tauri/src/launch.rs), commit `8c6f685a62a8e570f57e5072c01ee070fa4ef148`. Its installed ChatGPT/Codex app candidates also informed bundle discovery. The account module adapts its [Codex app-server JSON line transport](https://github.com/bartekczyz/ai-profiles/blob/8c6f685a62a8e570f57e5072c01ee070fa4ef148/apps/ai-profiles/src-tauri/src/codex_rpc.rs) to read each profile's account and rate limits. The upstream project is MIT licensed. This project does not include its credential, migration, session transfer, or launcher bundle code.

## MIT notices for adapted source

Copyright (c) 2026 Edi Hasaj

Copyright (c) 2026 Bartek Czyż

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

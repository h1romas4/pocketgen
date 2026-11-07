---
marp: true
title: Zephyr でつくるマイポケットコンピュータ
paginate: true
size: 16:9
style: |
  section {
    font-family: "Noto Sans CJK JP", "Noto Sans JP", sans-serif;
    color: #172521;
    background: #f5f8f6;
    padding: 48px 64px;
  }
  h1, h2 { color: #114f49; }
  h1 { font-size: 58px; }
  h2 { font-size: 42px; margin-bottom: 24px; }
  section.title { background: #114f49; color: #f8fbf8; }
  section.title h1 { color: #fff; }
  section.title strong { color: #ffe277; }
  section.demo { background: #172521; color: #f8fbf8; }
  section.demo h1 { color: #ffe277; font-size: 88px; }
  pre { font-size: 24px; }
  code { font-family: "DejaVu Sans Mono", monospace; }
---

<!-- _class: title -->

# Zephyr でつくるマイポケットコンピュータ

ひろましゃ / @h1romas4

---

## 自己紹介

# ひろましゃ

**@h1romas4**

- 普段はインフラエンジニア
- 小さなコンピュータや、手元で動く機械が好き

---

## ポケコン的な機械がいろいろでております

- **[M5Stack Tab5](https://www.switch-science.com/products/11257?pr_prod_strat=similar&pr_rec_id=2ee4ed8b8&pr_rec_pid=8928023904454&pr_ref_pid=8497613570246&pr_seq=uniform)**：画面を持ち歩ける ESP32-P4 デバイス
- **[LILYGO T-Deck](https://lilygo.cc/products/t-deck)**：画面とキーボードを備えた小型デバイス
- **[関数電卓 CASIO fx-CG50](https://web.casio.jp/dentaku/fxcg50/index.html)**：小さな画面とキーで、計算もプログラムも

---

## 💡 そうだ、令和のポケコンをつくろう

大きな液晶を生かして左でぽちぽちプログラムを書いて、右ですぐにプログラムを実行できるような機械をつくりたい。

```text
+------------------------------+------------------------------+
| Edit                         | MicroPython                  |
+------------------------------+------------------------------+
| 1  print('Hello!')           | Hello!                       |
| 2  print(2 + 3)              | 5                            |
| 3  _                         |                              |
+------------------------------+------------------------------+
```

- いろいろな機械で動かしたい。
- できるだけ同じアプリのコードを使いたい
- 画面や入力の違いを、アプリから切り離せないかな？ ⇒ 💡Zephyr!

---

## Zephyr とは

- Linux Foundation 傘下のオープンソース RTOS プロジェクト
- 組み込み向けに、カーネルからドライバー、ネットワークまで提供
- 多様なボードや CPU アーキテクチャーに対応
- ビルド時に必要な機能とハードウェア構成を選ぶ

参考: [Zephyr Project](https://www.zephyrproject.org/)

---

## Zephyr の広がり

- Arduino は Zephyr コアになりました
- 月面着陸機への応用
- 半導体メーカーや機器メーカーなど、多くの企業が参加

参考: [Arduino Core for Zephyr](https://github.com/zephyrproject-rtos/ArduinoCore-zephyr) / [宇宙機向け研究](https://zephyrproject.org/recap-open-source-summit-japan-2025/)

---

## ハードウェアの違いを記述する

**Devicetree** ファイルで「どのディスプレイを使うか」を選ぶ。

```dts
/* Zephyr の ESP32-P4 ボード定義から抜粋 */
/ {
    chosen {
        zephyr,display = &dsi_display;
    };
};
```

アプリは `zephyr,display` を参照。ボードごとの差を設定に分けられる。

出典: [Zephyr の ESP32-P4 ボード定義](https://github.com/zephyrproject-rtos/zephyr/blob/072fd9442fe12568bcc3d3c61e4f390ed5cf35a3/boards/espressif/esp32p4_function_ev_board/esp32p4_function_ev_board_hpcore.dts)

---

## やってみた：pocketgen

```text
pocketgen/
├── src/                 # Rust アプリ / C ブリッジ
├── components/
│   ├── pocketgen-editor/    # 編集コア
│   ├── pocketgen-terminal/  # 端末描画
│   └── micropython/         # MicroPython
├── boards/              # ボードごとの設定
│   ├── native_sim_native_64.overlay                      # native_sim 向け
│   └── esp32p4_function_ev_board_esp32p4_hpcore.overlay  # ESP32-P4 開発ボード向け
├── CMakeLists.txt       # Zephyr・C・Rust を接続
└── prj.conf             # Zephyr の機能設定
```

---

## ひとつのアプリ、ふたつの環境

```text
           pocketgen: エディタ + MicroPython
                            │
                  Zephyr display / UART API
                  ┌─────────┴─────────┐
            native_sim               ESP32-P4
          SDL + 仮想 UART          DSI 液晶 + USB
```

- native_sim：画面表示・編集・Python 実行を検証
- ESP32-P4：クロスビルド

---

## native_sim の特徴

- Zephyr アプリを **PC 上で実行**できる
- 実機がなくても、開発・デバッグ・自動テストができる
- pocketgen は SDL で画面を表示し、仮想 UART で入力
- 実機と同じ Zephyr API を使い、アプリの動作を確かめる

※ 実機固有のドライバーやタイミングは別途確認が必要

---

## MicroPython

- MicroPython は **Zephyr 向けのポート**を持つ
- pocketgen は MicroPython のコアを組み込み、入力したコードを実行

```c
mp_parse_tree_t tree = mp_parse(lexer, MP_PARSE_FILE_INPUT);
mp_obj_t function = mp_compile(&tree, source_name, false);
mp_call_function_0(function);
```

出典：[src/c/micropython.c](../src/c/micropython.c)

- 古の BASIC のような画面操作を `pocketgen` モジュールに追加

---

## BASIC 風に画面を操作する

```python
from pocketgen import cls, locate, color

cls()
color(14, 4)
locate(2, 1)
print("Hello, pocketgen!")
```

- `cls()`：右の画面を消去
- `locate(x, y)`：左上を `(0, 0)` として位置を指定
- `color(fg, bg)`：文字色と背景色を指定

---

## 実装例：`cls()` の正体

```c
static mp_obj_t pocketgen_cls(void) {
    mp_print_str(&mp_plat_print, "\x1b[2J\x1b[H");
    return mp_const_none;
}
static MP_DEFINE_CONST_FUN_OBJ_0(pocketgen_cls_obj, pocketgen_cls);
```

- `\x1b[2J` で画面消去、`\x1b[H` でカーソルを左上へ
- 出力された **ANSI エスケープシーケンス**を右側の端末が解釈
- C の関数を MicroPython から呼べる関数オブジェクトにする

出典：[src/c/micropython.c](../src/c/micropython.c)

---

## 位置と色も、端末への出力で制御

```c
mp_printf(&mp_plat_print, "\x1b[%u;%uH", (unsigned)y + 1, (unsigned)x + 1);
```

- `locate`：Python 側の 0 始まりを端末の **1 始まり**に変換

```c
mp_printf(&mp_plat_print, "\x1b[%u;%um", fg_code, bg_code);
```

- `color`：0〜15 の色番号を ANSI の前景色・背景色に変換
- 範囲外の座標や色は `ValueError` に

出典：[src/c/micropython.c](../src/c/micropython.c)（一部抜粋）

---

## C の関数を Python に公開

```c
static MP_DEFINE_CONST_FUN_OBJ_0(pocketgen_cls_obj, pocketgen_cls);

{ MP_ROM_QSTR(MP_QSTR_cls), MP_ROM_PTR(&pocketgen_cls_obj) },

MP_REGISTER_MODULE(MP_QSTR_pocketgen, pocketgen_module);
```

- C 関数を MicroPython の関数オブジェクトに変換
- 名前と関数を対応づけ、組み込みモジュールとして登録
- Python から `from pocketgen import cls` で利用できる

出典：[src/c/micropython.c](../src/c/micropython.c)（一部抜粋）

---

<!-- _class: demo -->

# デモ

エディタに書く → 実行する → 結果を見る

---

## おわりに

- 令和のポケコンっぽいものを、つくってみた
- いろいろな実機で動かしてみたい
- Zephyr 上でプログラムをかくと再利用性がぐんとあがる

[github.com/h1romas4/pocketgen](https://github.com/h1romas4/pocketgen)

---

## 補足①：FPU と Rust / C の ABI

液晶初期化で不正命令 → **FPU を有効にして ABI を統一**

```ini
CONFIG_FPU=y
CONFIG_FPU_SHARING=y
```

```cmake
set(RUST_TARGET "riscv32imafc-unknown-none-elf" PARENT_SCOPE)
```

- Rust と Zephyr / C を単精度 hard-float ABI **`ilp32f`** に合わせる
- 設定は [ボード設定](../boards/esp32p4_function_ev_board_esp32p4_hpcore.conf) と [CMakeLists.txt](../CMakeLists.txt)（抜粋）

---

## 補足②：Rust のメモリを PSRAM へ

内部 RAM のヒープが不足 → **外部 PSRAM の共有ヒープ**を使用

```rust
unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
    pocketgen_rust_alloc(layout.size(), layout.align())
}
```

```c
void *pocketgen_rust_alloc(size_t size, size_t align) {
    return shared_multi_heap_aligned_alloc(SMH_REG_ATTR_EXTERNAL, align, size);
}
```

- Rust のグローバルアロケータから C 経由で Zephyr の共有ヒープを利用
- 画面バッファも PSRAM に配置
- 出典：[allocator.rs](../src/allocator.rs) / [allocator.c](../src/c/allocator.c)

---

## 補足：その他の実機対応

- **入力が届かない**：コンソールを USB-UART につながる UART0 に変更
- 手元のチップは初期版 **v0.1**。動作は確認できたが、想定する v1.3 より古く、起動時の revision / ROM SHA-256 警告は残っている

設定と詳細：[README の ESP32-P4 節](../README.md#esp32-p4-setup)

---

## 補足：Zephyr Rust モジュールへのパッチ

ESP32-P4 のビルド時、上流モジュールを**ビルドディレクトリにコピー**して変更

- **Rust ターゲット**：`riscv32imafc-unknown-none-elf` に変更し、FPU 無効時はエラー
- **Kconfig 定数**：32-bit 環境で `usize` に収まらない値は `u64` として生成
- **アロケータ**：上流のグローバルアロケータを ESP32-P4 では無効化し、PSRAM 用の実装を使用

上流のチェックアウトは変更しない。パッチ対象が見つからなければビルドを停止。

出典：[CMakeLists.txt](../CMakeLists.txt)（ESP32-P4 向けの処理）

---

## 補足：Kconfig 定数の型

ESP32-P4 は 32-bit。`usize` は **`0xFFFFFFFF` まで**

```ini
CONFIG_SOC_GPIO_VALID_GPIO_MASK=0x007FFFFFFFFFFFFF
```

上流では 16 進数をすべて `usize` で生成 → **コンパイルエラー**

```rust
// パッチ後：32-bit の usize に収まらない値は u64
pub const CONFIG_SOC_GPIO_VALID_GPIO_MASK: u64 =
    0x007FFFFFFFFFFFFF;
```

出典：[CMakeLists.txt](../CMakeLists.txt)（Kconfig 定数生成のパッチ）

// owl page.js — ページへ常駐注入する UserScript(設計書 §9・§10)。
//
// document-start・全フレームで注入され(§10)、hint 機能と §10 の focus 監視(insert 自動移行)を
// 提供する。
//
// 役割分担(§9.1): クリック可能要素の列挙・ラベル採番・オーバーレイ描画・絞り込み・
// 確定時の click/focus は JS(このファイル)が担う。キー入力の受付とモード遷移は Rust 側。
// Rust → JS: owlHints.start()/input(ch)/cancel()(§9.2)。JS → Rust: script message handler
// "owl" へ JSON 文字列を postMessage(§9.2)。
(function () {
  "use strict";

  // §9.3: ラベル文字はホームロー(qutebrowser 既定)。
  var LABEL_CHARS = "sadfjklewcmpgh";
  // §9.3: 対象要素セレクタ。input[type=hidden] は下の可視判定で弾く。
  var TARGET_SELECTOR =
    "a[href], button, input, textarea, select, [onclick], [role=button], [role=link], [contenteditable]";
  var HINT_CLASS = "owl-hint";
  var STYLE_ID = "owl-hint-style";
  var CONTAINER_ID = "owl-hint-container";

  // hint セッションの状態。
  var state = { active: false, hints: [], buffer: "" };

  // §10: 直近のユーザー mousedown のタイムスタンプ(insert 自動移行の相関判定に使う)。
  var lastMouseDown = 0;
  // §10: mousedown から focusin までを「ユーザー起因の focus」とみなす窓(ミリ秒)。
  var FOCUS_WINDOW_MS = 200;

  // §9.2: JS → Rust。ハンドラ未登録(about: 等)でも例外にしない。
  function post(obj) {
    try {
      if (
        window.webkit &&
        window.webkit.messageHandlers &&
        window.webkit.messageHandlers.owl
      ) {
        window.webkit.messageHandlers.owl.postMessage(JSON.stringify(obj));
      }
    } catch (_e) {
      /* noop */
    }
  }

  // §9.3: テキスト入力欄(focus 対象)か、リンク相当(click 対象)かを判定する。
  function isEditable(el) {
    var tag = el.tagName.toLowerCase();
    if (tag === "textarea") return true;
    if (el.isContentEditable) return true;
    if (tag === "input") {
      var t = (el.getAttribute("type") || "text").toLowerCase();
      return (
        [
          "button",
          "submit",
          "reset",
          "checkbox",
          "radio",
          "file",
          "image",
          "hidden",
          "color",
          "range",
        ].indexOf(t) === -1
      );
    }
    return false;
  }

  // §9.3: ビューポート内かつ可視の要素だけを対象にする。
  function isVisible(el) {
    var rect = el.getBoundingClientRect();
    if (rect.width <= 0 || rect.height <= 0) return false;
    if (
      rect.bottom < 0 ||
      rect.right < 0 ||
      rect.top > window.innerHeight ||
      rect.left > window.innerWidth
    ) {
      return false;
    }
    var style = window.getComputedStyle(el);
    if (style.visibility === "hidden" || style.display === "none") return false;
    if (el.type && el.type.toLowerCase() === "hidden") return false;
    return true;
  }

  // §9.3: 要素数に応じて 1〜2 文字のラベル列を生成する(プレフィックス衝突が起きないよう
  // 全て同じ長さに揃える)。
  function makeLabels(n) {
    var labels = [];
    var chars = LABEL_CHARS.split("");
    if (n <= chars.length) {
      for (var i = 0; i < n; i++) labels.push(chars[i]);
      return labels;
    }
    for (var a = 0; a < chars.length; a++) {
      for (var b = 0; b < chars.length; b++) {
        if (labels.length >= n) return labels;
        labels.push(chars[a] + chars[b]);
      }
    }
    return labels;
  }

  function ensureStyle() {
    if (document.getElementById(STYLE_ID)) return;
    var style = document.createElement("style");
    style.id = STYLE_ID;
    style.textContent =
      "." +
      HINT_CLASS +
      "{position:fixed;z-index:2147483647;background:#f5d76e;color:#000;" +
      "font:bold 11px monospace;line-height:1.2;padding:0 2px;border:1px solid #a67c00;" +
      "border-radius:2px;box-shadow:0 1px 2px rgba(0,0,0,.4);}";
    (document.head || document.documentElement).appendChild(style);
  }

  // §9.4: オーバーレイを全除去し、状態をリセットする。
  function cleanup() {
    var container = document.getElementById(CONTAINER_ID);
    if (container) container.remove();
    var style = document.getElementById(STYLE_ID);
    if (style) style.remove();
    state.active = false;
    state.hints = [];
    state.buffer = "";
  }

  // §9.2: owlHints.start() — 要素列挙とラベル表示。
  function start() {
    cleanup();
    ensureStyle();

    var elements = [];
    var nodeList = document.querySelectorAll(TARGET_SELECTOR);
    for (var i = 0; i < nodeList.length; i++) {
      if (isVisible(nodeList[i])) elements.push(nodeList[i]);
    }

    // §9.2: 候補 0 件は hint_none(Rust は Normal へ戻る)。about:blank 等もここで無害に終わる。
    if (elements.length === 0) {
      post({ type: "hint_none" });
      return;
    }

    // ラベルは最大 14 + 14*14 = 196 個。これを超える要素はラベル付けできないため切り詰める
    // (MVP: 超過分はヒント対象外。§9.3)。切り詰めないと labels[j] が undefined になり
    // toUpperCase() で例外 → オーバーレイ未表示・hint_none も送られず Hint にスタックする。
    var labels = makeLabels(elements.length);
    if (elements.length > labels.length) {
      elements = elements.slice(0, labels.length);
    }
    var container = document.createElement("div");
    container.id = CONTAINER_ID;

    state.hints = [];
    state.buffer = "";
    for (var j = 0; j < elements.length; j++) {
      var el = elements[j];
      var label = labels[j];
      var rect = el.getBoundingClientRect();
      var tag = document.createElement("div");
      tag.className = HINT_CLASS;
      tag.textContent = label.toUpperCase();
      tag.style.left = Math.max(0, rect.left) + "px";
      tag.style.top = Math.max(0, rect.top) + "px";
      container.appendChild(tag);
      state.hints.push({ label: label, el: el, tag: tag });
    }
    document.documentElement.appendChild(container);
    state.active = true;
  }

  // 対象を確定する: リンクは click(SPA ハンドラも動く)、テキスト入力欄は focus(§9.3)。
  // オーバーレイを先に除去してから実行し、結果を Rust へ通知する(§9.2)。
  function activate(hint) {
    var el = hint.el;
    var editable = isEditable(el);
    cleanup();
    if (editable) {
      el.focus();
      post({ type: "hint_result", target: "input" });
    } else {
      el.click();
      post({ type: "hint_result", target: "link" });
    }
  }

  // §9.2: owlHints.input(ch) — ラベル文字の追加入力(絞り込み・確定判定)。
  function input(ch) {
    if (!state.active) return;
    state.buffer += String(ch).toLowerCase();

    var matches = state.hints.filter(function (h) {
      return h.label.indexOf(state.buffer) === 0;
    });

    // §9.2: 絞り込みで全滅 → hint_none。
    if (matches.length === 0) {
      cleanup();
      post({ type: "hint_none" });
      return;
    }

    // ラベル全長一致 → 確定(ラベルは同長なので一致すれば一意)。
    var exact = matches.find(function (h) {
      return h.label === state.buffer;
    });
    if (exact) {
      activate(exact);
      return;
    }

    // まだ確定しない: 一致するものだけ表示し、外れたものは隠す。
    var matched = {};
    matches.forEach(function (h) {
      matched[h.label] = true;
    });
    state.hints.forEach(function (h) {
      h.tag.style.display = matched[h.label] ? "" : "none";
    });
  }

  // §9.2: owlHints.cancel() — オーバーレイ除去(Esc キャンセル時に Rust から呼ぶ)。
  function cancel() {
    cleanup();
  }

  window.owlHints = { start: start, input: input, cancel: cancel };

  // §10: insert 自動移行。ユーザー操作起因の focus でのみ owl へ通知し、`autofocus`・スクリプト
  // 起因(`element.focus()`)では通知しない(要求 3.3)。capture でページより先に観測する。
  //
  // 相関判定: mousedown のタイムスタンプを記録し、focusin が直近 FOCUS_WINDOW_MS 以内かつ対象が
  // editable(§10: `isEditable`)のときのみ `{"type":"focus","editable":true}` を送る。owl は Normal
  // のときだけこれを受けて Insert へ遷移する(§10、検証は Rust 側)。
  //
  // hint 確定の `.focus()`(§9)はスクリプト起因で mousedown を伴わないため、ここでは通知されない
  // (二重遷移しない)。hint 側は自前の `hint_result:input` で Insert へ遷移する。
  document.addEventListener(
    "mousedown",
    function () {
      lastMouseDown = Date.now();
    },
    true,
  );
  document.addEventListener(
    "focusin",
    function (e) {
      if (Date.now() - lastMouseDown > FOCUS_WINDOW_MS) return;
      if (!e.target || !isEditable(e.target)) return;
      post({ type: "focus", editable: true });
    },
    true,
  );
})();

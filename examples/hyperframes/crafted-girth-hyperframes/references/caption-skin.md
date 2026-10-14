# Caption sub-composition (distilled from the coral caption-skin)

Wrap this in `<template>` in `compositions/captions.html`. Restyle colours/fonts freely; keep the hooks (`.caption-group`, `.caption-word`, `.is-active`, `.is-spoken`).

```html
<template>
<script src="https://cdn.jsdelivr.net/npm/gsap@3.14.2/dist/gsap.min.js"></script>
<style>
  #captions-root { position:absolute; inset:0; pointer-events:none; }
  .caption-stage { position:absolute; left:0; right:0; top:900px; height:180px;
    display:flex; align-items:center; justify-content:center; }
  .caption-group { position:absolute; inset:0; display:flex; align-items:center;
    justify-content:center; opacity:0; }
  .caption-line { display:flex; flex-wrap:wrap; justify-content:center; gap:0.1em 0.3em;
    font-size:56px; line-height:1.04; text-transform:uppercase; letter-spacing:0.06em; }
  .caption-word { display:inline-block; padding:0 0.12em; color:#8a857c; }
  .caption-word.is-active { color:#1a1a1a; background:#e85d5d; box-shadow:0 0 0 0.1em #e85d5d; }
  .caption-word.is-spoken { color:#1a1a1a; background:transparent; box-shadow:none; }
</style>
<div id="captions-root" data-composition-id="captions" data-timeline-locked
     data-start="0" data-duration="20" data-fps="30" data-width="1920" data-height="1080">
  <div id="caption-stage" class="caption-stage"></div>
</div>
<script>
  var GROUPS = [/* {start,end,words:[{text,start,end}]} */];
  var DURATION = 20;
  (function () {
    var stage = document.getElementById("caption-stage");
    GROUPS.forEach(function (group, g) {
      var ge = document.createElement("div");
      ge.className = "caption-group"; ge.id = "caption-group-" + g;
      var line = document.createElement("div"); line.className = "caption-line";
      group.words.forEach(function (w, i) {
        var s = document.createElement("span");
        s.className = "caption-word"; s.id = "caption-word-" + g + "-" + i;
        s.textContent = String(w.text); line.appendChild(s);
      });
      ge.appendChild(line); stage.appendChild(ge);
    });
    window.__timelines = window.__timelines || {};
    var tl = gsap.timeline({ paused: true });
    GROUPS.forEach(function (group, g) {
      var ge = document.getElementById("caption-group-" + g), words = group.words;
      var next = GROUPS[g + 1], isLast = g === GROUPS.length - 1;
      var start = Math.max(0, Number(group.start));
      var end = isLast ? DURATION : Math.min(Number(next.start), Number(group.end) + 0.3);
      if (end <= start) end = start + 0.01;
      tl.set(ge, { opacity: 1 }, start);
      tl.set(ge, { opacity: 0 }, end);
      words.forEach(function (w, i) {
        var el = document.getElementById("caption-word-" + g + "-" + i);
        var at = Math.max(start, Number(w.start));
        tl.set(el, { className: "caption-word" }, start);
        tl.set(el, { className: "caption-word is-active" }, at);
        tl.fromTo(el, { scale: 0.97 }, { scale: 1, duration: 0.14, ease: "power2.out" }, at);
        if (i + 1 < words.length)
          tl.set(el, { className: "caption-word is-spoken" }, Math.max(start, Number(words[i + 1].start)));
      });
      var last = document.getElementById("caption-word-" + g + "-" + (words.length - 1));
      tl.set(last, { className: "caption-word is-spoken" }, Math.min(end, Number(words[words.length - 1].end) + 0.1));
    });
    tl.to({}, { duration: DURATION }, 0);
    window.__timelines["captions"] = tl;
  })();
</script>
</template>
```

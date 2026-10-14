# Five evidence views

[Open the comparison](index.html) · [Report](../../2026-09-29-navigation-and-provenance.md) · [Corpus](corpus.json) · [Coverage evaluation](evaluation.json)

The comparison is local and contains only public/constructed research material. It uses local files for its image, video and evidence links. Original private histories and their URLs are absent. No remote script, model call, acquisition callback or automatic publication is built into this page.

From the repository root, rebuild the corpus, set-coverage metrics and HTML:

```sh
python3 docs/research/expanded-exploration/04-navigation-and-provenance/experiments/build_navigation.py
```

Open `index.html` in a browser, or serve the **expanded-exploration directory** so sibling concern assets resolve:

```sh
python3 -m http.server 8876 --bind 127.0.0.1 --directory docs/research/expanded-exploration
```

Then open `http://127.0.0.1:8876/04-navigation-and-provenance/experiments/navigation-lab/index.html`.

`template.html` is the editable source. `corpus.json`, `evaluation.json` and `index.html` are deterministic derived artifacts. The data includes acquisition state, epistemic status, origin, source/observation dates, source hashes and region/page/time anchors where available. Filter defaults are laboratory comparison settings, not accepted product behavior.

The eight task records are authored required-ID sets. Full set coverage is not a claim of successful UI navigation, answer accuracy or usability. Authored question groups are oracle grouping. The 24-versus-85 item comparison has explicit assumptions in `evaluation.json`. Current browser smoke-check logs verify a limited set of actual tab/filter changes using an owned browser; they do not measure preferences or general accessibility.

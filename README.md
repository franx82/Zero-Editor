<p align="center">
  <img src="assets/icon.png" width="140" height="140" alt="Zero Editor Logo" />
</p>

<h1 align="center">Zero Editor</h1>

<p align="center">
  <strong>Un editor di testo minimalista nativo per macOS, scritto in Rust.</strong><br>
  Ispirato all'essenzialità e alla reattività di Zed, con <em>zero fronzoli</em>.
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Platform-macOS-000000?logo=apple" alt="macOS" />
  <img src="https://img.shields.io/badge/Language-Rust-orange?logo=rust" alt="Rust" />
  <img src="https://img.shields.io/badge/Theme-Nord-88c0d0" alt="Nord Theme" />
  <img src="https://img.shields.io/badge/GUI-iced%200.14-blue" alt="iced" />
  <img src="https://img.shields.io/badge/License-MIT-green" alt="MIT" />
</p>

---

## ⚡ Filosofia

- **Zero Fronzoli**: Nessuna telemetria, nessun server remoto, nessun account, nessun login.
- **Nessuna AI Invasiva**: Nessun assistente che distrae o riscrive il codice in background.
- **Velocità & Metal GPU**: Motore grafico hardware-accelerato su Apple Silicon / Metal a 60/120Hz ProMotion via `iced` e `wgpu`.
- **Persistenza Totale**: Puoi chiudere l'app, riavviare o spegnere il Mac in qualsiasi momento. Tutti i tab aperti, l'ordine, la riga/colonna del cursore e persino i buffer non salvati (*drafts*) vengono ripristinati istantaneamente alla riapertura.

---

## ✨ Caratteristiche

### 1. Persistenza Automatica della Sessione
- Salvataggio continuo in `~/Library/Application Support/dev.zero.editor/session.json`.
- Buffer modificati o non ancora salvati archiviati come bozze in `drafts/`: nessuna perdita di dati accidentale.

### 2. Estetica & Stile Nord (Arctic Dark)
- Palette ufficiale **Nord**:
  - Superficie editor: `nord0` (`#2e3440`)
  - Header e Status bar: `nord1` (`#3b4252`)
  - Tab attivi / hover: `nord2` (`#434c5e`)
  - Bordi e testi secondari: `nord3` (`#4c566a`) / `nord4` (`#d8dee9`)
  - Accenti e selezioni: `nord8` (`#88c0d0` Frost Ice)
  - Pallino file modificato: `nord12` (`#d08770` Aurora Orange)
- Title bar nativa macOS: rispetta l'aspetto chiaro/scuro del sistema operativo per la barra del titolo, mantenendo il tema Nord uniforme all'interno.

### 3. Syntax Highlighting Modulare (Oltre 900 Linguaggi)
- Bundled con il motore di sintassi di `two-face` / `syntect` (**919 definizioni di sintassi pronte all'uso**).
- **Rilevamento automatico**:
  - Dall'estensione (`.rs`, `.py`, `.ts`, `.tsx`, `.js`, `.go`, `.c`, `.cpp`, `.zig`, `.swift`, `.json`, `.toml`, `.yaml`, `.sql`, `.md`, ecc.).
  - Dal nome del file (`Dockerfile`, `Makefile`, `Cargo.toml`, `Cargo.lock`, `.gitignore`, `CMakeLists.txt`).
  - Dallo shebang iniziale (`#!/usr/bin/env python`, `#!/bin/bash`, `#!/usr/bin/env node`, ecc.).
- **Language Picker Rapido (`⌘ + L`)**: menu fluttuante di ricerca per selezionare o cambiare la sintassi in qualsiasi momento.
- Tasto `ON / OFF` nella status bar per disattivare l'evidenziazione con un clic.

### 4. Dot per Spazi e Indicatori Tab
- Micro-font vettoriale TrueType incorporato (`assets/ZeroWhitespace.ttf`, <1KB) a zero costo di runtime e con precisione monospazio al pixel (`8.40px`).
- I caratteri effettivi rimangono puri spazi e tab nel testo sorgente (nessun carattere strano salvato su disco o nei compilatori).
- Attivabile/disattivabile al volo con **`⌘ + ⇧ + D`** (o **`⌘ + .`**) o dal pulsante **`· Dot: ON / OFF`** nella status bar.

### 5. Incolonnamento Automatico Intelligente (Smart Indent)
- Mantiene l'indentazione precedente quando premi `Invio`.
- Indentazione automatica a blocchi (+4 spazi) dopo `{`, `(`, `[`, o `:`.
- Espansione automatica tra parentesi `{}`: premendo `Invio` con il cursore tra `{` e `}` crea 3 righe, indentando quella centrale con il cursore pronto e chiudendo la graffa all'indentazione di partenza.
- **Tab**: indenta la selezione o inserisce spazi allineati al prossimo tab stop di 4 colonne.
- **⇧ + Tab**: de-indenta la riga o la selezione.
- **Smart Backspace**: cancella 4 spazi di indentazione in un unico colpo.

---

## ⌨️ Scorciatoie da Tastiera

| Scorciatoia | Azione |
| :--- | :--- |
| `⌘ + N` / `⌘ + T` | Nuovo tab |
| `⌘ + O` | Apri file (dialog nativo macOS) |
| `⌘ + S` | Salva file corrente |
| `⌘ + ⇧ + S` | Salva con nome |
| `⌘ + W` | Chiudi tab attivo |
| `⌘ + L` | Apri Language Picker / selettore sintassi |
| `Esc` | Chiudi il Language Picker |
| `⌘ + ⇧ + D` / `⌘ + .` | Toggle visualizzazione dot spazi / tab |
| `Tab` | Indenta riga / selezione (4 spazi) |
| `⇧ + Tab` | De-indenta riga / selezione |
| `⌘ + 1` .. `⌘ + 9` | Seleziona tab per indice |
| `⌃ + Tab` / `⌘ + ⇧ + ]` | Tab successivo |
| `⌃ + ⇧ + Tab` / `⌘ + ⇧ + [` | Tab precedente |
| **Drag & Drop** | Trascina qualsiasi file dal Finder per aprirlo |

---

## 🚀 Compilazione ed Esecuzione

### Prerequisiti
- macOS (Apple Silicon o Intel)
- Toolchain Rust installata (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)

### Esecuzione con Cargo
```bash
# Avvia l'editor
cargo run --release

# Apri file specifici direttamente da terminale
cargo run --release -- src/main.rs Cargo.toml
```

### Creazione del pacchetto macOS nativo (`.app`)
Esegui lo script di packaging incluso:
```bash
./scripts/build_app.sh
```

Verrà generato il bundle **`target/Zero Editor.app`** completo di icona nativa Retina e metadata.  
Puoi spostarlo direttamente nella cartella Applicazioni:
```bash
cp -r "target/Zero Editor.app" /Applications/
```

---

## 📂 Struttura del Progetto

```text
Zero-Editor/
├── assets/
│   ├── icon.icns               # Icona nativa multi-risoluzione macOS
│   ├── icon.png                # Icona master 1024x1024
│   └── ZeroWhitespace.ttf      # Micro-font per rendering whitespace
├── scripts/
│   ├── build_app.sh            # Script per compilare e impacchettare l'app macOS
│   ├── generate_icon.py        # Generatore icona .png e .icns
│   └── generate_whitespace_font.py # Generatore font per spazi/tab
├── src/
│   ├── highlighter.rs          # Integrazione two-face e rendering dot spazi/tab
│   ├── main.rs                 # Stato dell'app, ciclo di eventi, interfaccia iced
│   ├── session.rs              # Gestore persistenza sessione e bozze non salvate
│   ├── tab.rs                  # Gestione buffer, auto-indentazione e cursore
│   └── theme.rs                # Palette colori ufficiale Nord e stili widget
└── Cargo.toml
```

---

## 📄 Licenza

Rilasciato sotto licenza MIT.

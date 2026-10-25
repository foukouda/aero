# Step 1 specification (original, in French)

This is the original requirements document for step 1, kept verbatim.
Decisions taken afterwards are listed at the end.

---

# Projet : réécriture moderne de XFOIL en Rust, avec interface Tauri — Étape 1 : la base

## Contexte
Je construis un outil d'analyse et, à terme, d'optimisation de profils d'aile, inspiré de XFOIL (Mark Drela) et XFLR5. Objectif final : un solveur plus robuste, parallèle et utilisable comme bibliothèque, avec une interface moderne. Cette première étape pose uniquement les fondations : géométrie, solveur non visqueux, interface graphique. Le code doit être propre, testé et prêt à évoluer.

## Périmètre de cette étape
INCLUS :
1. Géométrie des profils
2. Solveur non visqueux fidèle à XFOIL
3. Application de bureau Tauri pour visualiser et piloter le tout

EXCLU pour l'instant (mais l'architecture doit les permettre plus tard) :
- couche limite et calcul visqueux (Cd, transition)
- optimisation de forme
- partie 3D (LLT, VLM)
- liaisons Python, version WASM

## Architecture imposée
Workspace Cargo :

```
aero/
  crates/
    geometry/      # profils, splines, panélisation, paramétrisations
    foil2d/        # solveur 2D (module inviscid maintenant, viscous plus tard)
  app/
    src-tauri/     # couche Tauri fine : commandes qui appellent les crates
    src/           # frontend
```

Règles :
- Les crates de calcul ne dépendent JAMAIS de Tauri. Elles doivent être utilisables seules (CLI, Python, WASM plus tard).
- Aucun état global. Le solveur est un objet indépendant (ex. `InviscidSolver::new(&airfoil, &options)`) pour pouvoir lancer plusieurs calculs en parallèle plus tard.
- `f64` partout.
- Pas de `unwrap()`/`expect()`/`panic!` dans les bibliothèques : erreurs via `Result` et `thiserror`.
- Types d'entrée/sortie sérialisables avec `serde`.
- Code formaté (`rustfmt`) et sans avertissement (`cargo clippy -- -D warnings`).
- Fonctions publiques documentées.

## 1. Crate `geometry`
- Struct `Airfoil` : nom + coordonnées (x, y), ordonnées du bord de fuite extrados → bord d'attaque → bord de fuite intrados (convention Selig).
- Import de fichiers `.dat` aux formats Selig ET Lednicer (détection automatique), robuste aux lignes vides, espaces et en-têtes variés.
- Export `.dat` au format Selig.
- Générateurs NACA 4 et 5 chiffres, bord de fuite fermé ou ouvert en option.
- Paramétrisation CST (Kulfan), ordre réglable, avec conversion profil → coefficients CST par moindres carrés (servira à l'optimisation).
- Spline cubique paramétrée par l'abscisse curviligne, comme `SPLINE`/`SEVAL` de XFOIL.
- Détection du bord d'attaque par le point de courbure / distance max au bord de fuite (comme `LEFIND`).
- Normalisation (corde = 1, bord d'attaque en (0,0)).
- Repanélisation équivalente à `PANGEN` de XFOIL : nombre de panneaux réglable (160 par défaut), resserrement réglable au bord d'attaque et au bord de fuite, basé sur la courbure.
- Grandeurs géométriques : épaisseur max et sa position, cambrure max et sa position, rayon de bord d'attaque, angle de bord de fuite, épaisseur du bord de fuite.

## 2. Crate `foil2d` — module `inviscid`
Reproduire la formulation de XFOIL (`xpanel.f` : `PSILIN`, `GGCALC`) :
- Méthode de panneaux à vorticité linéaire par panneau, fonction de courant constante sur le profil.
- Panneau de bord de fuite avec source et vorticité pour les bords de fuite épais, comme XFOIL.
- Condition de Kutta.
- Deux solutions de base (α = 0° et α = 90°) calculées une seule fois par géométrie, puis superposées pour tout α. La matrice ne doit être factorisée qu'une fois par géométrie.
- Correction de compressibilité de Kármán-Tsien optionnelle (Mach en entrée), avec erreur explicite si le Mach critique est dépassé.
- Sorties : Cp sur chaque nœud, Cl (intégration des pressions), Cm au quart de corde, Cp min et sa position, vitesses à la paroi.
- Balayage en α (début, fin, pas) qui réutilise la factorisation.
- Algèbre linéaire avec `faer` (LU dense).
- Conception pensée pour l'ajout futur du couplage visqueux : les matrices d'influence doivent être accessibles.

## 3. Validation (obligatoire)
- Tests unitaires pour chaque fonction de géométrie.
- Cas de référence théoriques : profil symétrique à α = 0 → Cl = 0 ; pente de portance proche de 2π/rad pour un profil mince ; cylindre ou ellipse comparés à la solution analytique.
- Comparaison avec XFOIL en mode inviscide sur au moins NACA 0012, NACA 2412, NACA 4415 et un profil importé (ex. Clark Y), à plusieurs α. Je fournirai les sorties de référence de XFOIL dans `tests/reference/` ; prévoir le format et le code de comparaison.
- Comparaisons avec tolérance relative (pas d'égalité stricte, les résultats peuvent varier selon l'architecture).
- Tests de convergence en maillage (le Cl doit converger quand le nombre de panneaux augmente).
- Benchmarks `criterion` : temps de factorisation et temps par α.

## 4. Application Tauri
Stack : Tauri 2, Svelte 5 + TypeScript + Vite, graphiques avec uPlot.
- Types TypeScript générés automatiquement depuis les structs Rust (`ts-rs` ou `tauri-specta`). Aucun type dupliqué à la main.
- Calculs lancés hors du thread principal (`spawn_blocking`). L'interface ne doit jamais se figer.
- Progression envoyée via `tauri::ipc::Channel` pour les balayages, avec possibilité d'annuler.

Fonctionnalités :
- Bibliothèque de profils de la session : générer un NACA (4 ou 5 chiffres), importer un `.dat` (bouton + glisser-déposer), dupliquer, renommer, supprimer.
- Vue du profil : forme à l'échelle réelle (axes égaux), affichage optionnel des nœuds de panneaux, superposition de plusieurs profils.
- Panneau de propriétés géométriques (épaisseur, cambrure, etc.).
- Réglages de panélisation (nombre de panneaux, resserrements) avec aperçu immédiat.
- Analyse à un α : curseur α mis à jour en temps réel, courbe de Cp avec la convention aéro (−Cp vers le haut), affichage Cl, Cm, Cp min.
- Balayage en α : courbes Cl(α) et Cm(α), plusieurs profils superposés.
- Champ Mach optionnel.
- Export : profil en `.dat`, résultats en CSV, graphiques en PNG/SVG.
- Sauvegarde et ouverture de projet en JSON (profils + réglages + résultats), avec un numéro de version de format.
- Thème clair et sombre.
- Messages d'erreur clairs pour l'utilisateur (fichier invalide, géométrie dégénérée, Mach trop élevé…).

## 5. Qualité et outillage
- Git dès le début, commits petits et clairs à chaque étape.
- CI GitHub Actions : `fmt`, `clippy`, `test` sur Linux, macOS et Windows.
- README : installation, lancement, structure du projet.
- Licence : XFOIL est sous GPL. Si le code reprend directement sa formulation, le projet est sous GPL-2.0-or-later.

## Méthode de travail attendue
1. Commence par me proposer un plan détaillé découpé en étapes, sans coder.
2. Ordre conseillé : `geometry` → `inviscid` + tests de validation → squelette Tauri → fonctionnalités de l'interface.
3. À chaque étape : code, tests qui passent, clippy propre, commit. Ne passe pas à l'étape suivante si les tests échouent.
4. Si un point de la formulation de XFOIL est ambigu, signale-le et demande plutôt que de deviner.

## Critères de fin d'étape
- `cargo test` et `cargo clippy -- -D warnings` passent sur tout le workspace.
- Le solveur non visqueux reproduit XFOIL en inviscide sur les cas de référence, dans les tolérances.
- L'application se lance, importe un profil, affiche sa forme, son Cp en temps réel et ses courbes Cl/Cm, et exporte les résultats.

---

## Decisions taken after the plan review (2026-10-25)

- Code, documentation, commits, README and user interface are in English.
- Reference XFOIL version: **6.996** (latest), built from source in CI
  (gfortran, double precision).
- The repository is a private GitHub repository.
- Critical Mach: error by default when Cp_min < Cp*; an `allow_supercritical`
  option restores XFOIL's behaviour (warning only, error only when the
  Kármán–Tsien correction breaks down). In a sweep, failed points are flagged
  without aborting the sweep.
- Paneling: the node count follows XFOIL's `PPAR N` convention (nodes, not
  panels), 160 by default as specified. XFOIL 6.996 itself defaults to 240
  nodes and a TE/LE density ratio of 0.3; both presets are available.
- The workspace root is the repository root (it plays the role of `aero/`);
  packages are named `aero-geometry` and `aero-foil2d`.
- Analyses run on coordinates as given (like XFOIL); normalization is an
  explicit operation. Thickness and camber follow XFOIL's definition
  (chord-line axes, same chordwise abscissa on both surfaces).

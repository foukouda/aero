Airfoil analysis and shape optimization — XFOIL 6.996 rewritten in Rust, validated against it.

## Download (Windows 10 / 11, 64-bit)

| File | What it is |
|---|---|
| **`Aero_{{VERSION}}_x64-setup.exe`** | Installer (recommended): installs Aero for your user account, no administrator rights needed, with a Start-menu shortcut and an uninstaller. |
| `Aero_{{VERSION}}_x64_portable.exe` | The application alone: nothing to install, run it from anywhere (a USB stick…). |
| `SHA256SUMS.txt` | Checksums to verify the downloads. |

**Windows SmartScreen.** The files are not signed with a paid code-signing
certificate, so Windows may show “Windows protected your PC” the first time.
Click **More info**, then **Run anyway**. The files are built from this
repository's source by the public [release workflow](https://github.com/foukouda/aero/actions/workflows/release.yml);
compare their checksums with `SHA256SUMS.txt` if in doubt.

Aero uses Microsoft Edge WebView2, included in Windows 10 (recent updates)
and 11; the installer adds it if it is missing.

## Téléchargement (Windows 10 / 11, 64 bits)

- **`Aero_{{VERSION}}_x64-setup.exe`** : l'installateur (recommandé), sans droits administrateur, avec raccourci dans le menu Démarrer et désinstallation.
- `Aero_{{VERSION}}_x64_portable.exe` : l'application seule, sans installation.

Si Windows affiche « Windows a protégé votre ordinateur » (les fichiers ne
sont pas signés par un certificat payant), cliquez sur **Informations
complémentaires**, puis **Exécuter quand même**.

## In the application

Geometry (NACA generator, `.dat` import, PANGEN paneling), XFOIL's
geometry tools (flap deflection and series, thickness and camber, their high
points, trailing-edge gap, leading-edge radius, blending of two airfoils),
inviscid and viscous analysis at prescribed α or Cl, polars of many airfoils
computed in parallel and compared in a table, shape optimization, inverse
design (draw the surface speed you want: mixed inverse QDES or full inverse
MDES), a guided tour and a glossary in English and French. See the [README](https://github.com/foukouda/aero#readme).

// Steps of the guided tour ("discovery mode"). Pure data, rendered by
// components/Tour.svelte. Targets are `data-tour` attributes in the UI.

export type TourLang = "en" | "fr";
export type TourTab = "geometry" | "analysis" | "polars";
export type TourAction = "generate-naca" | "show-nodes" | "set-alpha" | "run-sweep";

export interface TourText {
  title: string;
  body: string;
  /** Label of the "do it for me" button, when the step has an action. */
  action?: string;
}

export interface TourStep {
  id: string;
  /** `data-tour` value of the highlighted element; none for a centered card. */
  target?: string;
  /** Tab shown during the step. */
  tab?: TourTab;
  /** The step only makes sense once an airfoil exists. */
  needsAirfoil?: boolean;
  action?: TourAction;
  text: Record<TourLang, TourText>;
}

export const TOUR_STEPS: TourStep[] = [
  {
    id: "welcome",
    text: {
      en: {
        title: "Welcome to Aero",
        body: "Aero analyses 2D airfoils with the inviscid panel method of XFOIL, rewritten in Rust and validated against XFOIL 6.996. This short tour walks you through a complete analysis: create an airfoil, check its geometry, compute its pressure distribution and its polar. You can try everything while the tour is open.",
      },
      fr: {
        title: "Bienvenue dans Aero",
        body: "Aero analyse des profils d'aile 2D avec la méthode de panneaux non visqueuse de XFOIL, réécrite en Rust et validée contre XFOIL 6.996. Cette visite te guide à travers une analyse complète : créer un profil, vérifier sa géométrie, calculer sa répartition de pression puis sa polaire. Tu peux tout essayer pendant que la visite est ouverte.",
      },
    },
  },
  {
    id: "naca",
    target: "naca",
    action: "generate-naca",
    text: {
      en: {
        title: "1. Create an airfoil",
        body: "Type a NACA designation — 4 digits (e.g. 2412: 2% camber at 40% chord, 12% thick) or 5 digits (e.g. 23012) — and press Generate. Tick “Closed trailing edge” for a sharp trailing edge.",
        action: "Generate a NACA 2412 for me",
      },
      fr: {
        title: "1. Créer un profil",
        body: "Tape une désignation NACA — 4 chiffres (ex. 2412 : cambrure 2 % à 40 % de la corde, épaisseur 12 %) ou 5 chiffres (ex. 23012) — puis clique sur Generate. Coche « Closed trailing edge » pour un bord de fuite aigu.",
        action: "Génère un NACA 2412 pour moi",
      },
    },
  },
  {
    id: "import",
    target: "import",
    text: {
      en: {
        title: "2. Or import a coordinate file",
        body: "Import any .dat file in Selig or Lednicer format (the UIUC database uses both) with this button, or simply drop files onto the window. Aero detects the format, fixes the point order and tells you what it corrected.",
      },
      fr: {
        title: "2. Ou importer un fichier de coordonnées",
        body: "Importe n'importe quel fichier .dat au format Selig ou Lednicer (la base UIUC utilise les deux) avec ce bouton, ou glisse simplement les fichiers sur la fenêtre. Aero détecte le format, remet les points dans le bon ordre et te signale ce qu'il a corrigé.",
      },
    },
  },
  {
    id: "library",
    target: "airfoil-list",
    needsAirfoil: true,
    text: {
      en: {
        title: "3. Your airfoil library",
        body: "Click an airfoil to select it. The checkbox shows or hides it in every plot, so you can compare several airfoils. Hover a line for its actions: rename (or double-click the name), duplicate, export as .dat, delete.",
      },
      fr: {
        title: "3. Ta bibliothèque de profils",
        body: "Clique sur un profil pour le sélectionner. La case à cocher l'affiche ou le masque dans tous les graphiques, pour comparer plusieurs profils. Survole une ligne pour ses actions : renommer (ou double-clic sur le nom), dupliquer, exporter en .dat, supprimer.",
      },
    },
  },
  {
    id: "tabs",
    target: "tabs",
    text: {
      en: {
        title: "4. Three views",
        body: "Geometry shows the shape and the paneling, Analysis computes one angle of attack, Polars sweeps a range of angles. The library on the left stays available in all of them.",
      },
      fr: {
        title: "4. Trois vues",
        body: "Geometry montre la forme et la panélisation, Analysis calcule un angle d'attaque, Polars balaie une plage d'angles. La bibliothèque à gauche reste disponible partout.",
      },
    },
  },
  {
    id: "shape",
    target: "shape",
    tab: "geometry",
    needsAirfoil: true,
    action: "show-nodes",
    text: {
      en: {
        title: "5. The shape, at true scale",
        body: "Both axes use the same scale, so the airfoil is not distorted. Turn on “Show panel nodes” to see the points used by the solver: they are packed near the leading and trailing edges where the flow changes fastest.",
        action: "Show the panel nodes",
      },
      fr: {
        title: "5. La forme, à l'échelle réelle",
        body: "Les deux axes ont la même échelle : le profil n'est pas déformé. Active « Show panel nodes » pour voir les points utilisés par le solveur : ils sont resserrés près du bord d'attaque et du bord de fuite, là où l'écoulement varie le plus vite.",
        action: "Affiche les nœuds",
      },
    },
  },
  {
    id: "properties",
    target: "properties",
    tab: "geometry",
    needsAirfoil: true,
    text: {
      en: {
        title: "6. Geometric properties",
        body: "Maximum thickness and camber (with their chordwise position), leading-edge radius, trailing-edge angle and thickness. If the chord is not 1 or the leading edge is not at the origin, use Normalize: coefficients are referred to a unit chord, as in XFOIL.",
      },
      fr: {
        title: "6. Propriétés géométriques",
        body: "Épaisseur et cambrure maximales (avec leur position le long de la corde), rayon de bord d'attaque, angle et épaisseur du bord de fuite. Si la corde n'est pas 1 ou si le bord d'attaque n'est pas à l'origine, utilise Normalize : les coefficients sont rapportés à une corde unité, comme dans XFOIL.",
      },
    },
  },
  {
    id: "paneling",
    target: "paneling",
    tab: "geometry",
    needsAirfoil: true,
    text: {
      en: {
        title: "7. Paneling",
        body: "The airfoil is cut into panels exactly like XFOIL's PANE command. More nodes give more accurate results (160 is a good default). Bunching packs nodes where the curvature is high; the TE/LE ratio refines the trailing edge. The view updates immediately.",
      },
      fr: {
        title: "7. Panélisation",
        body: "Le profil est découpé en panneaux exactement comme la commande PANE de XFOIL. Plus de nœuds donnent des résultats plus précis (160 est un bon choix par défaut). Le resserrement concentre les nœuds là où la courbure est forte ; le ratio BF/BA raffine le bord de fuite. La vue se met à jour immédiatement.",
      },
    },
  },
  {
    id: "alpha",
    target: "alpha",
    tab: "analysis",
    needsAirfoil: true,
    action: "set-alpha",
    text: {
      en: {
        title: "8. Angle of attack",
        body: "Drag the slider: the pressure distribution and the coefficients update in real time (the matrix is factorized once per geometry, then each angle costs a few microseconds). You can also type a value.",
        action: "Set α = 5°",
      },
      fr: {
        title: "8. Angle d'attaque",
        body: "Fais glisser le curseur : la répartition de pression et les coefficients se mettent à jour en temps réel (la matrice n'est factorisée qu'une fois par géométrie, chaque angle coûte ensuite quelques microsecondes). Tu peux aussi taper une valeur.",
        action: "Mets α = 5°",
      },
    },
  },
  {
    id: "mach",
    target: "mach",
    tab: "analysis",
    text: {
      en: {
        title: "9. Mach number",
        body: "0 means incompressible flow. Above 0, the Kármán–Tsien compressibility correction is applied. If the flow becomes locally supersonic (critical Mach number exceeded), Aero reports an error instead of a doubtful result; tick “allow supercritical” to get XFOIL's behaviour.",
      },
      fr: {
        title: "9. Nombre de Mach",
        body: "0 correspond à un écoulement incompressible. Au-dessus, la correction de compressibilité de Kármán–Tsien est appliquée. Si l'écoulement devient localement supersonique (Mach critique dépassé), Aero signale une erreur plutôt qu'un résultat douteux ; coche « allow supercritical » pour retrouver le comportement de XFOIL.",
      },
    },
  },
  {
    id: "coefficients",
    target: "coefficients",
    tab: "analysis",
    needsAirfoil: true,
    text: {
      en: {
        title: "10. Coefficients",
        body: "Cl is the lift coefficient, Cm the pitching moment about the quarter chord, Cp min the lowest pressure coefficient and where it occurs. The analysis is inviscid: there is no friction, so the drag Cdp is close to zero (its small value measures the discretization error). CSV exports the pressure distribution.",
      },
      fr: {
        title: "10. Coefficients",
        body: "Cl est le coefficient de portance, Cm le moment de tangage au quart de corde, Cp min le plus petit coefficient de pression et sa position. L'analyse est non visqueuse : il n'y a pas de frottement, donc la traînée Cdp est quasi nulle (sa petite valeur mesure l'erreur de discrétisation). CSV exporte la répartition de pression.",
      },
    },
  },
  {
    id: "cp-plot",
    target: "cp-plot",
    tab: "analysis",
    needsAirfoil: true,
    text: {
      en: {
        title: "11. Pressure distribution",
        body: "Cp along the chord, upper surface solid, lower surface dashed. By aerodynamic convention negative Cp (suction) is plotted upwards; the area between the curves is the lift. With a Mach number, the dotted line is the sonic Cp*. PNG and SVG export the chart.",
      },
      fr: {
        title: "11. Répartition de pression",
        body: "Cp le long de la corde, extrados en trait plein, intrados en pointillés. Par convention aérodynamique, le Cp négatif (dépression) est tracé vers le haut ; l'aire entre les courbes correspond à la portance. Avec un Mach, la ligne pointillée est le Cp* sonique. PNG et SVG exportent le graphique.",
      },
    },
  },
  {
    id: "sweep",
    target: "sweep",
    tab: "polars",
    needsAirfoil: true,
    action: "run-sweep",
    text: {
      en: {
        title: "12. Angle-of-attack sweep",
        body: "Choose the first and last angles and the step, then Run sweep: every visible airfoil is analyzed. Progress is shown and a long sweep can be cancelled at any time.",
        action: "Run the sweep for me",
      },
      fr: {
        title: "12. Balayage en angle d'attaque",
        body: "Choisis les angles de début et de fin et le pas, puis Run sweep : chaque profil visible est analysé. La progression s'affiche et un long balayage peut être annulé à tout moment.",
        action: "Lance le balayage pour moi",
      },
    },
  },
  {
    id: "polar-plots",
    target: "polar-plots",
    tab: "polars",
    needsAirfoil: true,
    text: {
      en: {
        title: "13. Polars",
        body: "Cl(α) and Cm(α) for every visible airfoil, overlaid for comparison. In inviscid flow the lift keeps growing linearly (about 2π per radian): there is no stall, which requires a viscous analysis. Export CSV saves all the points.",
      },
      fr: {
        title: "13. Polaires",
        body: "Cl(α) et Cm(α) de chaque profil visible, superposés pour comparer. En non visqueux, la portance croît linéairement (environ 2π par radian) : il n'y a pas de décrochage, qui nécessite une analyse visqueuse. Export CSV enregistre tous les points.",
      },
    },
  },
  {
    id: "project",
    target: "project",
    text: {
      en: {
        title: "14. Save your work",
        body: "Save stores the airfoils, settings and results in a project file (Ctrl+S); Open restores it. The last button switches between light and dark themes.",
      },
      fr: {
        title: "14. Enregistrer ton travail",
        body: "Save enregistre les profils, les réglages et les résultats dans un fichier projet (Ctrl+S) ; Open le rouvre. Le dernier bouton bascule entre thème clair et sombre.",
      },
    },
  },
  {
    id: "done",
    text: {
      en: {
        title: "You are ready",
        body: "That's the whole workflow. Replay this tour at any time with the Tour button at the top right.",
      },
      fr: {
        title: "C'est parti",
        body: "Tu connais tout le déroulement. Tu peux relancer cette visite à tout moment avec le bouton Tour en haut à droite.",
      },
    },
  },
];

/** Default tour language from the system locale. */
export function defaultTourLang(locale: string | undefined): TourLang {
  return locale?.toLowerCase().startsWith("fr") ? "fr" : "en";
}

export interface Box {
  left: number;
  top: number;
  width: number;
  height: number;
}

/**
 * Position of the tour card: beside the highlighted element (right, left,
 * below, then above, whichever fits), always kept inside the viewport.
 * Centered when there is no target.
 */
export function placeCard(
  target: Box | null,
  card: { width: number; height: number },
  view: { width: number; height: number },
  gap = 14,
  margin = 12,
): { left: number; top: number } {
  const clampLeft = (x: number) => Math.max(margin, Math.min(x, view.width - card.width - margin));
  const clampTop = (y: number) => Math.max(margin, Math.min(y, view.height - card.height - margin));
  if (!target) {
    return { left: clampLeft((view.width - card.width) / 2), top: clampTop((view.height - card.height) / 2) };
  }
  const right = target.left + target.width + gap;
  const left = target.left - gap - card.width;
  const below = target.top + target.height + gap;
  const above = target.top - gap - card.height;
  if (right + card.width <= view.width - margin) return { left: right, top: clampTop(target.top) };
  if (left >= margin) return { left, top: clampTop(target.top) };
  if (below + card.height <= view.height - margin) return { left: clampLeft(target.left), top: below };
  if (above >= margin) return { left: clampLeft(target.left), top: above };
  // Large target (e.g. a plot filling the window): overlay its lower right corner.
  return {
    left: clampLeft(target.left + target.width - card.width - gap),
    top: clampTop(target.top + target.height - card.height - gap),
  };
}

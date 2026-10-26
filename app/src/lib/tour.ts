// Steps of the guided tour ("discovery mode"). Pure data, rendered by
// components/Tour.svelte. Targets are `data-tour` attributes in the UI.

export type TourLang = "en" | "fr";
export type TourTab = "geometry" | "analysis" | "polars" | "optimize";
export type TourAction = "generate-naca" | "show-nodes" | "set-alpha" | "enable-viscous" | "run-sweep";

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
        body: "Aero analyses 2D airfoils with XFOIL's methods — the panel method and the viscous boundary layer (transition, drag, stall) — rewritten in Rust and validated against XFOIL 6.996. This short tour walks you through a complete analysis: create an airfoil, check its geometry, compute its pressure distribution, boundary layer and polars, then optimize its shape. You can try everything while the tour is open.",
      },
      fr: {
        title: "Bienvenue dans Aero",
        body: "Aero analyse des profils d'aile 2D avec les méthodes de XFOIL — la méthode de panneaux et la couche limite visqueuse (transition, traînée, décrochage) — réécrites en Rust et validées contre XFOIL 6.996. Cette visite te guide à travers une analyse complète : créer un profil, vérifier sa géométrie, calculer sa répartition de pression, sa couche limite et ses polaires, puis optimiser sa forme. Tu peux tout essayer pendant que la visite est ouverte.",
      },
    },
  },
  {
    id: "glossary",
    target: "glossary",
    text: {
      en: {
        title: "New to aerodynamics?",
        body: "The Glossary button explains every quantity — Cl, Cd, L/D, Cm, Cp, the Reynolds number, transition, stall… — with typical values. Labels with a dotted underline open the matching explanation when you click them, and show a short definition when you hover them.",
      },
      fr: {
        title: "Pas aérodynamicien ?",
        body: "Le bouton Glossary explique chaque grandeur — Cl, Cd, finesse L/D, Cm, Cp, nombre de Reynolds, transition, décrochage… — avec des valeurs typiques. Les libellés soulignés en pointillés ouvrent l'explication correspondante quand tu cliques dessus, et montrent une définition courte au survol.",
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
        body: "Type a NACA designation — 4 digits (e.g. 2412: 2% camber at 40% chord, 12% thick) or 5 digits (e.g. 23012) — and press Generate. Several at once: type a list such as 0012, 2412, 4415. Tick “Closed trailing edge” for a sharp trailing edge.",
        action: "Generate a NACA 2412 for me",
      },
      fr: {
        title: "1. Créer un profil",
        body: "Tape une désignation NACA — 4 chiffres (ex. 2412 : cambrure 2 % à 40 % de la corde, épaisseur 12 %) ou 5 chiffres (ex. 23012) — puis clique sur Generate. Plusieurs d'un coup : tape une liste comme 0012, 2412, 4415. Coche « Closed trailing edge » pour un bord de fuite aigu.",
        action: "Génère un NACA 2412 pour moi",
      },
    },
  },
  {
    id: "import",
    target: "import",
    text: {
      en: {
        title: "2. Or import coordinate files",
        body: "Import .dat files in Selig or Lednicer format (the UIUC database uses both) with this button, a whole folder of them with Folder…, or simply drop files or folders onto the window. Aero detects the format, fixes the point order and tells you what it corrected. Examples adds eight classic airfoils to compare (NACA sections, Clark Y, Eppler 387).",
      },
      fr: {
        title: "2. Ou importer des fichiers de coordonnées",
        body: "Importe des fichiers .dat au format Selig ou Lednicer (la base UIUC utilise les deux) avec ce bouton, tout un dossier avec Folder…, ou glisse simplement fichiers ou dossiers sur la fenêtre. Aero détecte le format, remet les points dans le bon ordre et te signale ce qu'il a corrigé. Examples ajoute huit profils classiques à comparer (NACA, Clark Y, Eppler 387).",
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
        body: "Drag the slider: the pressure distribution and the coefficients update as you move it. An inviscid point costs a few microseconds; a viscous point takes a few Newton iterations of the coupled boundary layer (a fraction of a second). You can also type a value. Switch α / Cl to prescribe the lift coefficient instead: the solver then finds the angle of attack (XFOIL's CL command), handy to compare airfoils at the same lift.",
        action: "Set α = 5°",
      },
      fr: {
        title: "8. Angle d'attaque",
        body: "Fais glisser le curseur : la répartition de pression et les coefficients suivent le mouvement. Un point non visqueux coûte quelques microsecondes ; un point visqueux demande quelques itérations de Newton de la couche limite couplée (une fraction de seconde). Tu peux aussi taper une valeur. Bascule α / Cl pour imposer plutôt le coefficient de portance : le solveur trouve alors l'angle d'attaque (commande CL de XFOIL), pratique pour comparer des profils à portance égale.",
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
    id: "viscous",
    target: "viscous",
    tab: "analysis",
    action: "enable-viscous",
    text: {
      en: {
        title: "10. Viscous analysis",
        body: "With Viscous ticked, Aero solves the boundary layer coupled with the panel method, exactly like XFOIL's VISC mode: friction, drag, laminar-to-turbulent transition and stall. Re is the chord Reynolds number (type 1e6, 500k…). Ncrit sets the transition sensitivity (top / bottom): 9 for an average wind tunnel, 4–6 for a turbulent free stream, 11–12 for a very quiet one. A trip x/c below 1 forces transition there, like a turbulator strip. “M, Re” lets the Mach and Reynolds numbers vary with the lift as for an aircraft of fixed wing loading (XFOIL TYPE 2: the values entered are then those at Cl = 1), and “Ampl.” selects XFOIL's modified amplification rate (DAMP).",
        action: "Enable it with Re = 1e6",
      },
      fr: {
        title: "10. Analyse visqueuse",
        body: "Avec Viscous coché, Aero résout la couche limite couplée à la méthode de panneaux, exactement comme le mode VISC de XFOIL : frottement, traînée, transition laminaire-turbulent et décrochage. Re est le nombre de Reynolds sur la corde (tape 1e6, 500k…). Ncrit règle la sensibilité de la transition (extrados / intrados) : 9 pour une soufflerie moyenne, 4–6 pour un écoulement turbulent, 11–12 pour un écoulement très calme. Un trip x/c inférieur à 1 force la transition à cet endroit, comme une bande de turbulateur. « M, Re » fait varier les nombres de Mach et de Reynolds avec la portance, comme pour un avion à charge alaire fixe (TYPE 2 de XFOIL : les valeurs saisies sont alors celles à Cl = 1), et « Ampl. » choisit le taux d'amplification modifié de XFOIL (DAMP).",
        action: "Active-la avec Re = 1e6",
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
        title: "11. Coefficients",
        body: "Cl is the lift coefficient, Cd the drag coefficient and L/D the lift-to-drag ratio, Cm the pitching moment about the quarter chord. Xtr gives where the boundary layer becomes turbulent on each surface (ᵗ when forced by a trip); Cdp / Cdf split the drag into pressure and friction. “not converged” flags a point (often beyond stall) whose values are approximate. In inviscid mode only Cl, Cm and a near-zero Cdp are available. The CSV buttons export the Cp distribution and the boundary layer. Tick “Flap hinge” and give the hinge point to get the hinge moment Ch of the flap aft of it (XFOIL FMOM).",
      },
      fr: {
        title: "11. Coefficients",
        body: "Cl est le coefficient de portance, Cd celui de traînée et L/D la finesse, Cm le moment de tangage au quart de corde. Xtr indique où la couche limite devient turbulente sur chaque face (ᵗ si elle est forcée par un trip) ; Cdp / Cdf séparent la traînée de pression et de frottement. « not converged » signale un point (souvent au-delà du décrochage) dont les valeurs sont approximatives. En non visqueux, seuls Cl, Cm et un Cdp quasi nul sont disponibles. Les boutons CSV exportent la répartition de Cp et la couche limite. Coche « Flap hinge » et donne le point de charnière pour obtenir le moment de charnière Ch du volet situé en arrière (FMOM de XFOIL).",
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
        title: "12. Pressure distribution",
        body: "Cp along the chord, upper surface solid, lower surface dashed. By aerodynamic convention negative Cp (suction) is plotted upwards; the area between the curves is the lift. In viscous mode the thin dotted lines are the inviscid Cp: the gap shows the effect of the boundary layer (a plateau marks a laminar separation bubble). With a Mach number, the grey line is the sonic Cp*. PNG and SVG export the chart.",
      },
      fr: {
        title: "12. Répartition de pression",
        body: "Cp le long de la corde, extrados en trait plein, intrados en tirets. Par convention aérodynamique, le Cp négatif (dépression) est tracé vers le haut ; l'aire entre les courbes correspond à la portance. En visqueux, les fines lignes pointillées sont le Cp non visqueux : l'écart montre l'effet de la couche limite (un plateau signale un bulbe de décollement laminaire). Avec un Mach, la ligne grise est le Cp* sonique. PNG et SVG exportent le graphique.",
      },
    },
  },
  {
    id: "bl-plot",
    target: "bl-plot",
    tab: "analysis",
    needsAirfoil: true,
    text: {
      en: {
        title: "13. Boundary layer",
        body: "Choose a quantity: displacement and momentum thicknesses δ* and θ, shape factor H (above about 3.5–4 the laminar layer separates), skin friction Cf (negative where the flow is separated), amplification N (transition where it reaches Ncrit) or edge velocity. Top surface solid, bottom dashed, wake dotted.",
      },
      fr: {
        title: "13. Couche limite",
        body: "Choisis une grandeur : épaisseurs de déplacement et de quantité de mouvement δ* et θ, facteur de forme H (au-delà de 3,5–4 environ, la couche laminaire décolle), frottement Cf (négatif là où l'écoulement est décollé), amplification N (la transition a lieu quand elle atteint Ncrit) ou vitesse extérieure. Extrados en trait plein, intrados en tirets, sillage en pointillés.",
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
        title: "14. Angle-of-attack sweep",
        body: "Choose the first and last angles and the step, then Run sweep: every visible airfoil is analyzed, viscous or not according to the settings (the same as in the Analysis tab). As in XFOIL, each viscous point starts from the previous one, so start near zero lift and use small steps (0.5° to 1°) to follow the polar through stall. Progress is shown and a sweep can be cancelled at any time.",
        action: "Run the sweep for me",
      },
      fr: {
        title: "14. Balayage en angle d'attaque",
        body: "Choisis les angles de début et de fin et le pas, puis Run sweep : chaque profil visible est analysé, en visqueux ou non selon les réglages (les mêmes que dans l'onglet Analysis). Comme dans XFOIL, chaque point visqueux part du précédent : commence près de la portance nulle et utilise de petits pas (0,5° à 1°) pour suivre la polaire jusqu'au décrochage. La progression s'affiche et un balayage peut être annulé à tout moment.",
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
        title: "15. Polars",
        body: "Cl(α), Cm(α) and, for viscous polars, the drag polar Cl(Cd) and the transition points, for every visible airfoil (inviscid polars are dashed). The lift curve bends over at stall, the drag polar shows the laminar “bucket” and the transition curves explain it. Points that do not converge are left out, as in XFOIL. Export CSV saves all the points.",
      },
      fr: {
        title: "15. Polaires",
        body: "Cl(α), Cm(α) et, pour les polaires visqueuses, la polaire de traînée Cl(Cd) et les points de transition, pour chaque profil visible (les polaires non visqueuses sont en tirets). La courbe de portance s'infléchit au décrochage, la polaire de traînée montre le « baquet » laminaire et les courbes de transition l'expliquent. Les points non convergés sont écartés, comme dans XFOIL. Export CSV enregistre tous les points.",
      },
    },
  },
  {
    id: "polar-table",
    target: "polar-view",
    tab: "polars",
    needsAirfoil: true,
    text: {
      en: {
        title: "16. Compare many airfoils",
        body: "With many airfoils, sweep them all at once (they are computed in parallel, one per core) and switch to Table: zero-lift angle, Cl max, Cd min, best L/D and their Cl, and the values at the Cl you choose, side by side. Click a column's ⇅ to rank the airfoils, a row to select one; the table exports as CSV. Viscous sweeps start near 0 and march outward, so that a range starting in separated flow still gives a full polar.",
      },
      fr: {
        title: "16. Comparer beaucoup de profils",
        body: "Avec beaucoup de profils, lance le balayage sur tous d'un coup (ils sont calculés en parallèle, un par cœur) et passe en Table : angle de portance nulle, Cl max, Cd min, meilleure finesse L/D et leurs Cl, et les valeurs au Cl de ton choix, côte à côte. Clique sur le ⇅ d'une colonne pour classer les profils, sur une ligne pour en sélectionner un ; le tableau s'exporte en CSV. Les balayages visqueux partent de 0 et vont vers l'extérieur, pour qu'une plage commençant en écoulement décollé donne quand même une polaire complète.",
      },
    },
  },
  {
    id: "optimize-setup",
    target: "optimize-setup",
    tab: "optimize",
    needsAirfoil: true,
    text: {
      en: {
        title: "17. Optimize the shape",
        body: "The Optimize tab reshapes an airfoil for your operating points: each point prescribes Cl (or α) and has a weight; the objective is the weighted drag, or L/D. Constraints keep it buildable — minimum thickness (preset to the airfoil's own), thickness at a spar station, minimum Cm, trailing-edge angle. The shape is described by CST functions fitted to the airfoil, and every candidate is analyzed by the same viscous solver as elsewhere; a candidate that does not converge is rejected. The run is reproducible: the same seed gives the same result.",
      },
      fr: {
        title: "17. Optimiser la forme",
        body: "L'onglet Optimize remodèle un profil pour tes points de fonctionnement : chaque point impose Cl (ou α) et a un poids ; l'objectif est la traînée pondérée, ou L/D. Des contraintes le gardent réalisable — épaisseur minimale (préréglée sur celle du profil), épaisseur à l'emplacement d'un longeron, Cm minimal, angle de bord de fuite. La forme est décrite par des fonctions CST ajustées au profil, et chaque candidat est analysé par le même solveur visqueux qu'ailleurs ; un candidat qui ne converge pas est rejeté. Le calcul est reproductible : la même graine donne le même résultat.",
      },
    },
  },
  {
    id: "optimize-results",
    target: "optimize-results",
    tab: "optimize",
    needsAirfoil: true,
    text: {
      en: {
        title: "18. Follow and use the result",
        body: "During the run, the shape and the convergence plot update live (green: feasible designs, orange: designs violating a constraint, blue: best so far); Cancel keeps the best design found. The table compares the initial and optimized airfoils point by point. “Add to library” makes the optimized airfoil a regular one (analysis, export .dat), and “Compare polars” sweeps both at the optimization conditions — check the off-design behaviour there, a single-point optimum can be worse elsewhere.",
      },
      fr: {
        title: "18. Suivre et utiliser le résultat",
        body: "Pendant le calcul, la forme et la courbe de convergence se mettent à jour en direct (vert : formes admissibles, orange : formes violant une contrainte, bleu : meilleure jusque-là) ; Cancel garde la meilleure forme trouvée. Le tableau compare le profil initial et le profil optimisé point par point. « Add to library » en fait un profil comme les autres (analyse, export .dat), et « Compare polars » calcule les polaires des deux aux conditions de l'optimisation — vérifie-y le comportement hors point de calcul : un optimum en un seul point peut être moins bon ailleurs.",
      },
    },
  },
  {
    id: "project",
    target: "project",
    text: {
      en: {
        title: "19. Save your work",
        body: "Save stores the airfoils, settings, results and the last optimization in a project file (Ctrl+S); Open restores it. The last button switches between light and dark themes.",
      },
      fr: {
        title: "19. Enregistrer ton travail",
        body: "Save enregistre les profils, les réglages, les résultats et la dernière optimisation dans un fichier projet (Ctrl+S) ; Open le rouvre. Le dernier bouton bascule entre thème clair et sombre.",
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

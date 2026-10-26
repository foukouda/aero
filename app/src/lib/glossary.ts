// Glossary of the quantities and terms used in the application, in
// English and French. Rendered by components/Glossary.svelte, used for the
// tooltips of components/Term.svelte, and written to docs/glossary.md and
// docs/glossaire.md (`npm run glossary`).

import type { TourLang } from "./tour";

export type GlossaryCategory =
  | "basics"
  | "aircraft"
  | "coefficients"
  | "flow"
  | "boundary-layer"
  | "geometry"
  | "options"
  | "optimization";

export interface GlossaryText {
  /** Name of the term. */
  term: string;
  /** One sentence, shown as a tooltip. */
  short: string;
  /** Full explanation (paragraphs separated by blank lines). */
  long: string;
}

export interface GlossaryEntry {
  id: string;
  /** Symbol as written in the interface, e.g. "Cl". */
  symbol?: string;
  category: GlossaryCategory;
  text: Record<TourLang, GlossaryText>;
}

export const CATEGORY_TITLES: Record<GlossaryCategory, Record<TourLang, string>> = {
  basics: { en: "Basics", fr: "Les bases" },
  aircraft: { en: "From the aircraft to the airfoil", fr: "De l'avion au profil" },
  coefficients: { en: "Forces and coefficients", fr: "Forces et coefficients" },
  flow: { en: "Flow conditions", fr: "Conditions d'écoulement" },
  "boundary-layer": { en: "Boundary layer and transition", fr: "Couche limite et transition" },
  geometry: { en: "Geometry", fr: "Géométrie" },
  options: { en: "Analysis options", fr: "Options d'analyse" },
  optimization: { en: "Shape optimization", fr: "Optimisation de forme" },
};

export const GLOSSARY: GlossaryEntry[] = [
  // ------------------------------------------------------------ basics
  {
    id: "airfoil",
    category: "basics",
    text: {
      en: {
        term: "Airfoil",
        short: "The cross-section of a wing, a propeller blade or a sail, cut parallel to the flow.",
        long: "An airfoil is the 2D shape you see when you cut a wing parallel to the flow: a rounded leading edge at the front, a sharp (or nearly sharp) trailing edge at the back, an upper and a lower surface.\n\nAero studies this 2D section, as if the wing were infinitely long: the results are per unit span. A real wing adds 3D effects (wing-tip vortices, induced drag) that are not computed here.",
      },
      fr: {
        term: "Profil",
        short: "La section d'une aile, d'une pale d'hélice ou d'une voile, coupée parallèlement à l'écoulement.",
        long: "Un profil est la forme 2D que l'on voit en coupant une aile parallèlement à l'écoulement : un bord d'attaque arrondi à l'avant, un bord de fuite aigu (ou presque) à l'arrière, un extrados (dessus) et un intrados (dessous).\n\nAero étudie cette section 2D, comme si l'aile était infiniment longue : les résultats sont par unité d'envergure. Une vraie aile ajoute des effets 3D (tourbillons marginaux, traînée induite) qui ne sont pas calculés ici.",
      },
    },
  },
  {
    id: "chord",
    symbol: "c",
    category: "basics",
    text: {
      en: {
        term: "Chord",
        short: "The length of the airfoil, from the leading edge to the trailing edge; positions are given as x/c.",
        long: "The chord c is the straight line from the leading edge to the trailing edge. Aero works with airfoils scaled to a unit chord, so a position x/c = 0.3 means 30 % of the chord from the leading edge, and a thickness t/c = 0.12 means 12 % of the chord.",
      },
      fr: {
        term: "Corde",
        short: "La longueur du profil, du bord d'attaque au bord de fuite ; les positions sont données en x/c.",
        long: "La corde c est la droite qui va du bord d'attaque au bord de fuite. Aero travaille sur des profils ramenés à une corde unité : une position x/c = 0,3 veut dire à 30 % de la corde depuis le bord d'attaque, et une épaisseur t/c = 0,12 veut dire 12 % de la corde.",
      },
    },
  },
  {
    id: "alpha",
    symbol: "α",
    category: "basics",
    text: {
      en: {
        term: "Angle of attack",
        short: "The angle between the chord and the oncoming flow, in degrees.",
        long: "The angle of attack α is the angle between the chord line and the direction of the oncoming air. Increasing it increases the lift, almost linearly (about +0.11 in Cl per degree), until the flow separates from the upper surface: that is the stall, usually between 10° and 18°.\n\nA cambered airfoil already lifts at α = 0; it reaches zero lift at a slightly negative angle (about −2° for a NACA 2412).",
      },
      fr: {
        term: "Angle d'attaque (incidence)",
        short: "L'angle entre la corde et l'écoulement qui arrive, en degrés.",
        long: "L'angle d'attaque α (ou incidence) est l'angle entre la corde et la direction de l'air qui arrive. L'augmenter augmente la portance, presque linéairement (environ +0,11 de Cl par degré), jusqu'à ce que l'écoulement se décolle de l'extrados : c'est le décrochage, en général entre 10° et 18°.\n\nUn profil cambré porte déjà à α = 0 ; il atteint la portance nulle à un angle légèrement négatif (environ −2° pour un NACA 2412).",
      },
    },
  },
  {
    id: "polar",
    category: "basics",
    text: {
      en: {
        term: "Polar",
        short: "The curves of an airfoil's coefficients over a range of angles of attack or lift coefficients.",
        long: "A polar describes the airfoil over its whole operating range: Cl(α) (lift curve), Cl(Cd) (drag polar), Cm(α) and the transition points. It is computed by a sweep, one point after the other.\n\nOn the drag polar Cl(Cd), the further left a curve is at a given Cl, the less drag the airfoil makes there. Laminar airfoils show a “drag bucket”: a range of Cl where the drag is particularly low.",
      },
      fr: {
        term: "Polaire",
        short: "Les courbes des coefficients d'un profil sur une plage d'angles d'attaque ou de coefficients de portance.",
        long: "Une polaire décrit le profil sur toute sa plage de fonctionnement : Cl(α) (courbe de portance), Cl(Cd) (polaire de traînée), Cm(α) et les points de transition. Elle est calculée par un balayage, point après point.\n\nSur la polaire de traînée Cl(Cd), plus une courbe est à gauche à un Cl donné, moins le profil traîne à ce Cl. Les profils laminaires montrent un « baquet » (drag bucket) : une plage de Cl où la traînée est particulièrement faible.",
      },
    },
  },
  {
    id: "sweep",
    category: "basics",
    text: {
      en: {
        term: "Sweep",
        short: "A series of analyses at regularly spaced angles of attack (or lift coefficients), which builds a polar.",
        long: "A sweep computes the airfoil at α = start, start + step, … up to the end value (or the same with Cl). In viscous mode each point starts from the boundary layer of the previous one, as XFOIL's ASEQ and CSEQ commands do: it is faster and converges more often. Points that do not converge (typically beyond the stall) are left out of the polar.",
      },
      fr: {
        term: "Balayage (sweep)",
        short: "Une série d'analyses à des angles d'attaque (ou des Cl) régulièrement espacés, qui construit une polaire.",
        long: "Un balayage calcule le profil à α = début, début + pas, … jusqu'à la valeur de fin (ou de même en Cl). En visqueux, chaque point part de la couche limite du point précédent, comme les commandes ASEQ et CSEQ de XFOIL : c'est plus rapide et ça converge plus souvent. Les points qui ne convergent pas (typiquement au-delà du décrochage) sont écartés de la polaire.",
      },
    },
  },
  {
    id: "xfoil",
    category: "basics",
    text: {
      en: {
        term: "XFOIL",
        short: "The reference airfoil analysis program by Mark Drela (MIT), which Aero rewrites and is validated against.",
        long: "XFOIL (1986–2013) is the most widely used airfoil analysis program, written in Fortran by Mark Drela at MIT. It couples a panel method for the outer flow with an integral boundary-layer model, which gives lift, drag, moment and transition in a fraction of a second.\n\nAero is a rewrite of XFOIL 6.996 in Rust; its results are checked against those of XFOIL on reference cases, down to the digits XFOIL prints.",
      },
      fr: {
        term: "XFOIL",
        short: "Le programme de référence d'analyse de profils de Mark Drela (MIT), qu'Aero réécrit et avec lequel il est validé.",
        long: "XFOIL (1986–2013) est le programme d'analyse de profils le plus utilisé, écrit en Fortran par Mark Drela au MIT. Il couple une méthode de panneaux pour l'écoulement extérieur à un modèle intégral de couche limite, ce qui donne portance, traînée, moment et transition en une fraction de seconde.\n\nAero est une réécriture de XFOIL 6.996 en Rust ; ses résultats sont comparés à ceux de XFOIL sur des cas de référence, jusqu'aux chiffres que XFOIL affiche.",
      },
    },
  },
  // ------------------------------------------------------ coefficients
  {
    id: "coefficient",
    category: "coefficients",
    text: {
      en: {
        term: "Dimensionless coefficient",
        short: "A force divided by the dynamic pressure and the chord, so that results do not depend on size or speed.",
        long: "Forces grow with the air density ρ, the square of the speed V and the size. Dividing a force per unit span by the dynamic pressure q = ½ρV² and by the chord c gives a coefficient that depends only on the shape, the angle of attack and the Reynolds and Mach numbers. For example the lift per metre of span is L = q · c · Cl.\n\nExample: at 20 m/s at sea level, q ≈ 245 Pa; a 0.2 m chord with Cl = 0.5 lifts 245 × 0.2 × 0.5 ≈ 25 N per metre of span.",
      },
      fr: {
        term: "Coefficient sans dimension",
        short: "Une force divisée par la pression dynamique et la corde, pour que les résultats ne dépendent ni de la taille ni de la vitesse.",
        long: "Les forces augmentent avec la masse volumique de l'air ρ, le carré de la vitesse V et la taille. En divisant une force par unité d'envergure par la pression dynamique q = ½ρV² et par la corde c, on obtient un coefficient qui ne dépend que de la forme, de l'angle d'attaque et des nombres de Reynolds et de Mach. Par exemple la portance par mètre d'envergure vaut L = q · c · Cl.\n\nExemple : à 20 m/s au niveau de la mer, q ≈ 245 Pa ; une corde de 0,2 m avec Cl = 0,5 porte 245 × 0,2 × 0,5 ≈ 25 N par mètre d'envergure.",
      },
    },
  },
  {
    id: "cl",
    symbol: "Cl",
    category: "coefficients",
    text: {
      en: {
        term: "Lift coefficient",
        short: "The lift (force perpendicular to the flow) in dimensionless form; higher means more lift.",
        long: "Cl measures the lift, the force perpendicular to the oncoming flow that holds an aircraft up. Typical values: 0 to 0.5 in cruise, 1.2 to 1.6 at the maximum for usual airfoils (Cl max, just before the stall), up to 2 or more for high-lift airfoils at low speed.\n\nCl grows almost linearly with the angle of attack (the theoretical slope is 2π per radian, about 0.11 per degree) until the stall, where it levels off and drops.",
      },
      fr: {
        term: "Coefficient de portance",
        short: "La portance (force perpendiculaire à l'écoulement) sous forme sans dimension ; plus il est grand, plus le profil porte.",
        long: "Cl mesure la portance, la force perpendiculaire à l'écoulement qui fait tenir un avion en l'air. Valeurs typiques : 0 à 0,5 en croisière, 1,2 à 1,6 au maximum pour des profils courants (Cl max, juste avant le décrochage), jusqu'à 2 ou plus pour des profils hypersustentateurs à basse vitesse.\n\nCl croît presque linéairement avec l'angle d'attaque (la pente théorique est 2π par radian, environ 0,11 par degré) jusqu'au décrochage, où il plafonne puis chute.",
      },
    },
  },
  {
    id: "cd",
    symbol: "Cd",
    category: "coefficients",
    text: {
      en: {
        term: "Drag coefficient",
        short: "The drag (force along the flow, resisting the motion) in dimensionless form; lower is better.",
        long: "Cd measures the drag, the force along the flow that the engine (or gravity, for a glider) must overcome. For a 2D airfoil it comes from friction on the surface and from the pressure difference created by the thickening boundary layer and the wake (see Cdp and Cdf).\n\nTypical values: 0.004 to 0.008 for a good airfoil in its normal range, more beyond the stall. Engineers often speak of drag counts: 1 count = 0.0001, so Cd = 0.0060 is 60 counts. Cd is only meaningful in viscous mode: an inviscid computation gives no drag.",
      },
      fr: {
        term: "Coefficient de traînée",
        short: "La traînée (force dans le sens de l'écoulement, qui freine) sous forme sans dimension ; plus il est petit, mieux c'est.",
        long: "Cd mesure la traînée, la force dans le sens de l'écoulement que le moteur (ou la gravité, pour un planeur) doit vaincre. Pour un profil 2D, elle vient du frottement sur la surface et de la différence de pression créée par l'épaississement de la couche limite et le sillage (voir Cdp et Cdf).\n\nValeurs typiques : 0,004 à 0,008 pour un bon profil dans sa plage normale, davantage au-delà du décrochage. On parle souvent en « points de traînée » (drag counts) : 1 point = 0,0001, donc Cd = 0,0060 fait 60 points. Cd n'a de sens qu'en mode visqueux : un calcul non visqueux ne donne aucune traînée.",
      },
    },
  },
  {
    id: "cdp-cdf",
    symbol: "Cdp / Cdf",
    category: "coefficients",
    text: {
      en: {
        term: "Pressure and friction drag",
        short: "The two parts of the drag: Cdf from skin friction, Cdp from the pressure field (form drag).",
        long: "Cdf is the friction of the air on the surface: it is low while the boundary layer is laminar and much higher once it is turbulent, which is why moving the transition back reduces drag. Cdp, the pressure (or form) drag, comes from the displacement of the flow by the boundary layer and grows strongly when the flow separates, near the stall.\n\nAt moderate angles, friction is usually the larger part.",
      },
      fr: {
        term: "Traînée de pression et de frottement",
        short: "Les deux parts de la traînée : Cdf due au frottement, Cdp due au champ de pression (traînée de forme).",
        long: "Cdf est le frottement de l'air sur la surface : il est faible tant que la couche limite est laminaire et bien plus fort une fois qu'elle est turbulente, d'où l'intérêt de reculer la transition. Cdp, la traînée de pression (ou de forme), vient du déplacement de l'écoulement par la couche limite et augmente fortement quand l'écoulement décolle, près du décrochage.\n\nAux angles modérés, le frottement est en général la plus grande part.",
      },
    },
  },
  {
    id: "ld",
    symbol: "L/D",
    category: "coefficients",
    text: {
      en: {
        term: "Lift-to-drag ratio (finesse)",
        short: "Cl divided by Cd: how much lift the airfoil gives per unit of drag; higher is better.",
        long: "L/D = Cl / Cd is the efficiency of the airfoil. Good airfoils reach 100 to 150 in 2D (a whole aircraft, with its fuselage and 3D effects, much less: about 15 for an airliner, 40 to 60 for a sailplane). Its maximum, and the Cl where it occurs, is a key figure when choosing an airfoil for gliding or long range.",
      },
      fr: {
        term: "Finesse",
        short: "Cl divisé par Cd : combien de portance le profil donne par unité de traînée ; plus elle est grande, mieux c'est.",
        long: "La finesse L/D = Cl / Cd mesure l'efficacité du profil. Les bons profils atteignent 100 à 150 en 2D (un avion complet, avec son fuselage et les effets 3D, beaucoup moins : environ 15 pour un avion de ligne, 40 à 60 pour un planeur). Son maximum, et le Cl où il se produit, est un chiffre clé pour choisir un profil pour le vol plané ou la distance.",
      },
    },
  },
  {
    id: "cm",
    symbol: "Cm",
    category: "coefficients",
    text: {
      en: {
        term: "Moment coefficient (about the quarter chord)",
        short: "The pitching moment about the point at 25 % of the chord; negative means the airfoil tends to pitch nose down.",
        long: "Cm measures the torque that the pressure exerts on the airfoil around a point at a quarter of the chord (c/4). For cambered airfoils it is negative and nearly constant before the stall: −0.05 for a NACA 2412, −0.1 or less for strongly cambered airfoils. A symmetric airfoil has Cm ≈ 0.\n\nIt matters for stability and structure: a strongly negative Cm needs a larger tail (or twist) to trim the aircraft and twists the wing; flying wings need airfoils with Cm ≥ 0 (reflexed trailing edge).",
      },
      fr: {
        term: "Coefficient de moment (au quart de corde)",
        short: "Le moment de tangage autour du point à 25 % de la corde ; négatif signifie que le profil tend à piquer.",
        long: "Cm mesure le couple que la pression exerce sur le profil autour d'un point situé au quart de la corde (c/4). Pour les profils cambrés il est négatif et presque constant avant le décrochage : −0,05 pour un NACA 2412, −0,1 ou moins pour des profils très cambrés. Un profil symétrique a Cm ≈ 0.\n\nIl compte pour la stabilité et la structure : un Cm très négatif demande un empennage plus grand (ou du vrillage) pour équilibrer l'avion et tord l'aile ; les ailes volantes demandent des profils à Cm ≥ 0 (bord de fuite relevé, « autostable »).",
      },
    },
  },
  {
    id: "cp",
    symbol: "Cp",
    category: "coefficients",
    text: {
      en: {
        term: "Pressure coefficient",
        short: "The pressure on the surface compared with the free-stream pressure; negative means suction.",
        long: "Cp = (p − p∞) / q tells how the local pressure p compares with the pressure far away p∞. Cp = 1 at the stagnation point (where the flow stops, on the leading edge), Cp < 0 where the air is faster than the free stream (suction, mostly on the upper surface).\n\nThe area between the upper and lower curves is the lift. By convention the plot has −Cp upwards, so that the upper surface (suction) is on top. A sharp suction peak followed by a steep rise of pressure promotes separation; Cp min is the lowest value.",
      },
      fr: {
        term: "Coefficient de pression",
        short: "La pression sur la surface comparée à la pression de l'écoulement libre ; négatif signifie dépression (aspiration).",
        long: "Cp = (p − p∞) / q indique comment la pression locale p se compare à la pression lointaine p∞. Cp = 1 au point d'arrêt (là où l'air s'arrête, au bord d'attaque), Cp < 0 là où l'air va plus vite que l'écoulement libre (dépression, surtout sur l'extrados).\n\nL'aire entre les courbes de l'extrados et de l'intrados est la portance. Par convention le graphique a −Cp vers le haut, pour que l'extrados (dépression) soit en haut. Un pic de dépression aigu suivi d'une forte remontée de pression favorise le décollement ; Cp min est la valeur la plus basse.",
      },
    },
  },
  {
    id: "stall",
    category: "coefficients",
    text: {
      en: {
        term: "Stall",
        short: "Beyond a certain angle of attack the flow separates from the upper surface: lift drops and drag soars.",
        long: "As the angle of attack grows, the pressure rise on the rear upper surface becomes too steep for the boundary layer, which separates. Lift stops increasing (Cl max) and drops, drag and moment change abruptly.\n\nStalled flows are unsteady and hard to compute: near and beyond Cl max, XFOIL's method (and Aero's) becomes approximate and often does not converge — such points are left out of the polars. Treat computed Cl max values as estimates.",
      },
      fr: {
        term: "Décrochage",
        short: "Au-delà d'un certain angle d'attaque, l'écoulement décolle de l'extrados : la portance chute et la traînée s'envole.",
        long: "Quand l'angle d'attaque augmente, la remontée de pression sur l'arrière de l'extrados devient trop forte pour la couche limite, qui décolle. La portance cesse d'augmenter (Cl max) puis chute, la traînée et le moment changent brutalement.\n\nLes écoulements décrochés sont instationnaires et difficiles à calculer : près et au-delà de Cl max, la méthode de XFOIL (et d'Aero) devient approximative et ne converge souvent pas — ces points sont écartés des polaires. Considère les Cl max calculés comme des estimations.",
      },
    },
  },
  {
    id: "hinge",
    symbol: "Ch",
    category: "coefficients",
    text: {
      en: {
        term: "Hinge moment",
        short: "The moment of the pressure on a flap (the part behind a hinge point) about that hinge.",
        long: "If the rear part of the airfoil is a control surface (flap, aileron, elevator) hinged at a point, the air pushes on it and creates a moment about the hinge; Ch is that moment divided by q·c². It tells how hard the actuator or the pilot must work. Here a positive Ch tends to raise the trailing edge of the flap (XFOIL's FMOM convention).",
      },
      fr: {
        term: "Moment de charnière",
        short: "Le moment de la pression sur un volet (la partie en arrière d'une charnière) autour de cette charnière.",
        long: "Si l'arrière du profil est une gouverne (volet, aileron, profondeur) articulée autour d'un point, l'air pousse dessus et crée un moment autour de la charnière ; Ch est ce moment divisé par q·c². Il indique l'effort que doit fournir le servo ou le pilote. Ici un Ch positif tend à relever le bord de fuite du volet (convention FMOM de XFOIL).",
      },
    },
  },
  // ------------------------------------------------------------ flow
  {
    id: "reynolds",
    symbol: "Re",
    category: "flow",
    text: {
      en: {
        term: "Reynolds number",
        short: "Speed × chord / viscosity of the air: compares inertia with viscosity; it sets the boundary-layer behaviour.",
        long: "Re = V · c / ν, where ν ≈ 1.5·10⁻⁵ m²/s for air at sea level. It is the most important parameter of a viscous analysis: at low Re the boundary layer stays laminar longer but separates easily (laminar bubbles), and the drag is higher; at high Re the flow is more turbulent and the airfoil reaches a higher Cl max.\n\nOrders of magnitude: model aircraft 50 000 – 300 000, sailplanes 1 – 3 million, light aircraft 3 – 6 million, airliners 20 – 50 million. Example: 20 m/s with a 0.2 m chord gives Re ≈ 270 000.",
      },
      fr: {
        term: "Nombre de Reynolds",
        short: "Vitesse × corde / viscosité de l'air : compare l'inertie à la viscosité ; il fixe le comportement de la couche limite.",
        long: "Re = V · c / ν, avec ν ≈ 1,5·10⁻⁵ m²/s pour l'air au niveau de la mer. C'est le paramètre le plus important d'une analyse visqueuse : à bas Re la couche limite reste laminaire plus longtemps mais décolle facilement (bulbes laminaires), et la traînée est plus forte ; à haut Re l'écoulement est plus turbulent et le profil atteint un Cl max plus élevé.\n\nOrdres de grandeur : modèles réduits 50 000 – 300 000, planeurs 1 – 3 millions, avions légers 3 – 6 millions, avions de ligne 20 – 50 millions. Exemple : 20 m/s avec une corde de 0,2 m donne Re ≈ 270 000.",
      },
    },
  },
  {
    id: "mach",
    symbol: "M",
    category: "flow",
    text: {
      en: {
        term: "Mach number",
        short: "Speed divided by the speed of sound; above about 0.3 the compressibility of the air starts to matter.",
        long: "M = V / a, with a ≈ 340 m/s at sea level. Below M ≈ 0.3 the air behaves as incompressible and M = 0 is fine. Above, the Kármán–Tsien correction accounts for compressibility, which increases the suction peaks.\n\nWhen the local flow becomes supersonic somewhere on the airfoil (Cp min below the critical value Cp*), shock waves appear, which this method cannot represent: Aero then reports that the critical Mach number is exceeded, unless “allow supercritical” is ticked (results then become unreliable).",
      },
      fr: {
        term: "Nombre de Mach",
        short: "La vitesse divisée par la vitesse du son ; au-dessus d'environ 0,3, la compressibilité de l'air commence à compter.",
        long: "M = V / a, avec a ≈ 340 m/s au niveau de la mer. En dessous de M ≈ 0,3, l'air se comporte comme incompressible et M = 0 convient. Au-dessus, la correction de Kármán–Tsien tient compte de la compressibilité, qui creuse les pics de dépression.\n\nQuand l'écoulement local devient supersonique quelque part sur le profil (Cp min en dessous de la valeur critique Cp*), des ondes de choc apparaissent, que cette méthode ne sait pas représenter : Aero signale alors que le Mach critique est dépassé, sauf si « allow supercritical » est coché (les résultats deviennent alors peu fiables).",
      },
    },
  },
  {
    id: "viscous",
    category: "flow",
    text: {
      en: {
        term: "Viscous and inviscid analysis",
        short: "Inviscid: ideal flow without friction (lift and moment only). Viscous: adds the boundary layer, hence drag, transition and stall.",
        long: "The inviscid analysis (panel method) treats the air as a perfect fluid. It is instantaneous and gives a good Cl and Cm at moderate angles, but no drag and no stall.\n\nThe viscous analysis couples it with a model of the thin layer of air slowed down by friction near the surface (the boundary layer), as XFOIL does. It gives the drag, the transition points and a realistic lift curve up to the stall. It needs a Reynolds number and takes a few tens of milliseconds per point.",
      },
      fr: {
        term: "Analyse visqueuse et non visqueuse",
        short: "Non visqueux : écoulement idéal sans frottement (portance et moment seulement). Visqueux : ajoute la couche limite, donc la traînée, la transition et le décrochage.",
        long: "L'analyse non visqueuse (méthode de panneaux) traite l'air comme un fluide parfait. Elle est instantanée et donne un bon Cl et un bon Cm aux angles modérés, mais aucune traînée et pas de décrochage.\n\nL'analyse visqueuse la couple à un modèle de la fine couche d'air freinée par le frottement près de la surface (la couche limite), comme XFOIL. Elle donne la traînée, les points de transition et une courbe de portance réaliste jusqu'au décrochage. Elle demande un nombre de Reynolds et prend quelques dizaines de millisecondes par point.",
      },
    },
  },
  // -------------------------------------------------- boundary layer
  {
    id: "boundary-layer",
    category: "boundary-layer",
    text: {
      en: {
        term: "Boundary layer",
        short: "The thin layer of air near the surface, slowed down by friction; it causes the drag and the stall.",
        long: "Right on the surface the air sticks to the wall (zero speed); a few millimetres away it moves at the outer speed. This thin layer is the boundary layer. It starts laminar (smooth, orderly) near the leading edge, then becomes turbulent (chaotic, with more friction) at the transition point. It continues behind the airfoil as the wake.\n\nIts plots in the Analysis tab: δ* (displacement thickness, how far the outer flow is pushed away), θ (momentum thickness, related to the drag), H = δ*/θ (shape factor: about 2.6 laminar, 1.4 turbulent; it rises toward separation, beyond about 3.5 laminar or 2.5 turbulent), Cf (skin friction), N (amplification of the disturbances, see Ncrit) and Ue (speed at the edge of the layer).",
      },
      fr: {
        term: "Couche limite",
        short: "La fine couche d'air près de la surface, freinée par le frottement ; elle cause la traînée et le décrochage.",
        long: "Au contact de la paroi, l'air colle (vitesse nulle) ; quelques millimètres plus loin, il va à la vitesse extérieure. Cette fine couche est la couche limite. Elle est d'abord laminaire (régulière) près du bord d'attaque, puis devient turbulente (désordonnée, avec plus de frottement) au point de transition. Elle se prolonge derrière le profil par le sillage.\n\nSes graphiques dans l'onglet Analysis : δ* (épaisseur de déplacement, de combien l'écoulement extérieur est repoussé), θ (épaisseur de quantité de mouvement, liée à la traînée), H = δ*/θ (facteur de forme : environ 2,6 en laminaire, 1,4 en turbulent ; il augmente à l'approche du décollement, au-delà d'environ 3,5 en laminaire ou 2,5 en turbulent), Cf (frottement pariétal), N (amplification des perturbations, voir Ncrit) et Ue (vitesse au bord de la couche).",
      },
    },
  },
  {
    id: "transition",
    symbol: "Xtr",
    category: "boundary-layer",
    text: {
      en: {
        term: "Transition",
        short: "Where the boundary layer turns from laminar to turbulent, as a fraction of the chord (Xtr), on each surface.",
        long: "Small disturbances in the laminar boundary layer grow until it becomes turbulent. Xtr top and Xtr bot give that position on the upper and lower surfaces (1 means laminar up to the trailing edge). Since turbulent friction is several times higher, a transition further back means less drag: that is the principle of laminar airfoils.\n\nThe transition moves forward on the upper surface and backward on the lower surface when the angle of attack increases. It is predicted with the eᴺ method (see Ncrit) or forced by a trip.",
      },
      fr: {
        term: "Transition",
        short: "L'endroit où la couche limite passe de laminaire à turbulente, en fraction de corde (Xtr), sur chaque face.",
        long: "De petites perturbations dans la couche limite laminaire grandissent jusqu'à la rendre turbulente. Xtr top et Xtr bot donnent cette position sur l'extrados et l'intrados (1 veut dire laminaire jusqu'au bord de fuite). Comme le frottement turbulent est plusieurs fois plus fort, une transition plus en arrière veut dire moins de traînée : c'est le principe des profils laminaires.\n\nLa transition avance sur l'extrados et recule sur l'intrados quand l'angle d'attaque augmente. Elle est prédite par la méthode eᴺ (voir Ncrit) ou forcée par un turbulateur (trip).",
      },
    },
  },
  {
    id: "ncrit",
    symbol: "Ncrit",
    category: "boundary-layer",
    text: {
      en: {
        term: "Critical amplification ratio",
        short: "How disturbed the incoming air is: the transition happens when disturbances have grown by e^Ncrit. 9 = average wind tunnel.",
        long: "The eᴺ method follows the growth of the disturbances in the laminar layer; N is the logarithm of their amplification. Transition happens when N reaches Ncrit, which depends on how calm the surrounding air is: 9 is the usual value (average wind tunnel), 11 to 14 for a sailplane in calm air, 4 to 8 for a noisy environment, a rough or vibrating surface.\n\nA lower Ncrit makes the transition happen earlier, hence more drag. Aero lets you set it separately on each surface.",
      },
      fr: {
        term: "Taux d'amplification critique",
        short: "À quel point l'air qui arrive est perturbé : la transition se produit quand les perturbations ont grandi d'un facteur e^Ncrit. 9 = soufflerie moyenne.",
        long: "La méthode eᴺ suit la croissance des perturbations dans la couche laminaire ; N est le logarithme de leur amplification. La transition se produit quand N atteint Ncrit, qui dépend du calme de l'air ambiant : 9 est la valeur habituelle (soufflerie moyenne), 11 à 14 pour un planeur en air calme, 4 à 8 pour un environnement bruyant, une surface rugueuse ou qui vibre.\n\nUn Ncrit plus faible fait transitionner plus tôt, donc plus de traînée. Aero permet de le régler séparément sur chaque face.",
      },
    },
  },
  {
    id: "trip",
    symbol: "Trip x/c",
    category: "boundary-layer",
    text: {
      en: {
        term: "Trip (forced transition)",
        short: "Forces the transition at a given x/c, like a turbulator strip or a rough edge. 1 = free transition.",
        long: "A trip forces the boundary layer to become turbulent at a given position, whatever the eᴺ method predicts — like a zig-zag tape, a step or dirt (insects) on a real wing. The transition then happens at the trip or earlier. Use it to model rough surfaces or to compare with wind-tunnel tests made with trips; 1 (the default) means free transition.",
      },
      fr: {
        term: "Turbulateur (transition forcée)",
        short: "Force la transition à un x/c donné, comme une bande de turbulateur ou un bord rugueux. 1 = transition libre.",
        long: "Un turbulateur (trip) force la couche limite à devenir turbulente à une position donnée, quoi que prédise la méthode eᴺ — comme un ruban zigzag, une marche ou des salissures (insectes) sur une vraie aile. La transition a alors lieu au turbulateur ou avant. Utile pour représenter des surfaces rugueuses ou comparer à des essais en soufflerie avec turbulateurs ; 1 (par défaut) veut dire transition libre.",
      },
    },
  },
  {
    id: "bubble",
    category: "boundary-layer",
    text: {
      en: {
        term: "Laminar separation bubble",
        short: "At low Reynolds numbers the laminar layer separates, turns turbulent and reattaches, leaving a small bubble that adds drag.",
        long: "Below Re ≈ 500 000 the laminar boundary layer often separates before it has become turbulent; the separated layer then turns turbulent and usually reattaches, enclosing a bubble. It shows as a plateau in Cp and a bump in δ* and H, and adds drag. XFOIL's method is known to predict these bubbles well — one of the reasons for its popularity for model aircraft and small drones.",
      },
      fr: {
        term: "Bulbe de décollement laminaire",
        short: "À bas nombre de Reynolds, la couche laminaire décolle, devient turbulente et recolle, laissant un petit bulbe qui ajoute de la traînée.",
        long: "En dessous de Re ≈ 500 000, la couche limite laminaire décolle souvent avant d'être devenue turbulente ; la couche décollée devient alors turbulente et recolle en général, en enfermant un bulbe. Il se voit par un plateau dans Cp et une bosse dans δ* et H, et ajoute de la traînée. La méthode de XFOIL est réputée bien prédire ces bulbes — une des raisons de son succès pour les modèles réduits et les petits drones.",
      },
    },
  },
  // ------------------------------------------------------- geometry
  {
    id: "naca",
    category: "geometry",
    text: {
      en: {
        term: "NACA designation",
        short: "Digits that describe a family of airfoils: NACA 2412 = 2 % camber at 40 % chord, 12 % thick.",
        long: "The NACA four-digit airfoils (1930s) are defined by formulas: in NACA MPTT, M is the maximum camber in % of the chord, P its position in tenths of the chord and TT the maximum thickness in %. NACA 0012 is symmetric and 12 % thick; NACA 4415 has 4 % camber at 40 % and is 15 % thick.\n\nFive-digit airfoils (e.g. 23012) have their maximum camber further forward and a small pitching moment: the first digit × 0.15 is the design Cl, the second gives the position of the maximum camber (in twentieths of the chord), the third says whether the mean line is reflexed (1) or not (0), and the last two give the thickness.",
      },
      fr: {
        term: "Désignation NACA",
        short: "Des chiffres qui décrivent une famille de profils : NACA 2412 = cambrure 2 % à 40 % de la corde, épaisseur 12 %.",
        long: "Les profils NACA à quatre chiffres (années 1930) sont définis par des formules : dans NACA MPTT, M est la cambrure maximale en % de la corde, P sa position en dixièmes de corde et TT l'épaisseur maximale en %. Le NACA 0012 est symétrique et épais de 12 % ; le NACA 4415 a 4 % de cambrure à 40 % et 15 % d'épaisseur.\n\nLes profils à cinq chiffres (ex. 23012) ont leur cambrure maximale plus en avant et un faible moment de tangage : le premier chiffre × 0,15 donne le Cl d'adaptation, le deuxième la position de la cambrure maximale (en vingtièmes de corde), le troisième indique si la ligne moyenne est relevée à l'arrière (1) ou non (0), et les deux derniers l'épaisseur.",
      },
    },
  },
  {
    id: "thickness",
    symbol: "t/c",
    category: "geometry",
    text: {
      en: {
        term: "Thickness",
        short: "The largest distance between the upper and lower surfaces, as a fraction of the chord.",
        long: "A thicker airfoil is stronger and stiffer (the spar is taller, the wing lighter) and holds more fuel, and it usually stalls more gently; a thinner one has less drag at high speed. Typical values: 8 – 10 % for fast or thin wings, 12 – 15 % for most aircraft, 18 – 21 % at the root of large wings. Its position (x/c of the maximum) matters for laminar flow.",
      },
      fr: {
        term: "Épaisseur",
        short: "La plus grande distance entre l'extrados et l'intrados, en fraction de la corde.",
        long: "Un profil plus épais est plus solide et plus rigide (le longeron est plus haut, l'aile plus légère) et contient plus de carburant, et il décroche en général plus doucement ; un profil plus mince traîne moins à grande vitesse. Valeurs typiques : 8 – 10 % pour des ailes rapides ou minces, 12 – 15 % pour la plupart des avions, 18 – 21 % à l'emplanture des grandes ailes. Sa position (x/c du maximum) compte pour l'écoulement laminaire.",
      },
    },
  },
  {
    id: "camber",
    category: "geometry",
    text: {
      en: {
        term: "Camber",
        short: "How curved the airfoil is: the largest distance between the chord and the mean line, as a fraction of the chord.",
        long: "The mean line runs halfway between the upper and lower surfaces; the camber is its largest distance to the chord. Camber gives lift at zero angle of attack and raises Cl max, at the cost of a more negative Cm. Symmetric airfoils (0 %) are used for tails and aerobatics; 2 – 4 % is common for wings, 5 – 8 % for low-speed high-lift airfoils.",
      },
      fr: {
        term: "Cambrure",
        short: "À quel point le profil est courbé : la plus grande distance entre la corde et la ligne moyenne, en fraction de corde.",
        long: "La ligne moyenne passe à mi-chemin entre l'extrados et l'intrados ; la cambrure est sa plus grande distance à la corde. La cambrure donne de la portance à incidence nulle et augmente Cl max, au prix d'un Cm plus négatif. Les profils symétriques (0 %) servent pour les empennages et la voltige ; 2 – 4 % est courant pour les ailes, 5 – 8 % pour des profils porteurs à basse vitesse.",
      },
    },
  },
  {
    id: "le-radius",
    category: "geometry",
    text: {
      en: {
        term: "Leading-edge radius",
        short: "The radius of the circle that fits the rounded nose, as a fraction of the chord.",
        long: "A larger leading-edge radius softens the suction peak at high angles of attack and gives a gentler stall; a small radius (sharp nose) stalls abruptly from the leading edge. For a NACA 0012 it is about 1.6 % of the chord.",
      },
      fr: {
        term: "Rayon de bord d'attaque",
        short: "Le rayon du cercle qui épouse le nez arrondi, en fraction de la corde.",
        long: "Un grand rayon de bord d'attaque adoucit le pic de dépression aux fortes incidences et donne un décrochage plus doux ; un petit rayon (nez pointu) décroche brutalement depuis le bord d'attaque. Pour un NACA 0012, il vaut environ 1,6 % de la corde.",
      },
    },
  },
  {
    id: "trailing-edge",
    category: "geometry",
    text: {
      en: {
        term: "Trailing-edge angle and thickness",
        short: "The angle between the surfaces at the trailing edge, and the gap between them if it is not sharp.",
        long: "A very small trailing-edge angle is hard to build and fragile; a blunt trailing edge (a small gap, often 0.1 – 0.5 % of the chord) is easier to manufacture and is modelled by XFOIL as a small panel across the gap.",
      },
      fr: {
        term: "Angle et épaisseur de bord de fuite",
        short: "L'angle entre les deux faces au bord de fuite, et l'écart entre elles s'il n'est pas aigu.",
        long: "Un angle de bord de fuite très petit est difficile à fabriquer et fragile ; un bord de fuite épais (un petit écart, souvent 0,1 – 0,5 % de la corde) est plus facile à réaliser et XFOIL le représente par un petit panneau en travers de l'écart.",
      },
    },
  },
  {
    id: "flap",
    category: "geometry",
    text: {
      en: {
        term: "Flap deflection",
        short: "The rear part of the airfoil is rotated about a hinge, positive down: more lift and more nose-down moment.",
        long: "A flap (or an aileron, an elevator) is the part of the airfoil behind a hinge. Deflecting it down by a few degrees cambers the airfoil: the lift at a given angle increases (roughly +0.05 to +0.1 in Cl per degree for a 25 % chord flap), the zero-lift angle moves to more negative values, and Cm becomes more negative. Deflected up, it does the opposite (reflex).\n\nThe hinge is given by its x position and either its y or its height as a fraction of the local thickness (0 on the lower surface, 1 on the upper one). As XFOIL does, the surface where the flap moves in is cut and a circular arc closes the gap on the other side; the rounded hinge of a real flap and the flow through its slot are not modelled.",
      },
      fr: {
        term: "Braquage de volet",
        short: "L'arrière du profil tourne autour d'une charnière, positif vers le bas : plus de portance et un moment plus piqueur.",
        long: "Un volet (ou un aileron, une gouverne de profondeur) est la partie du profil en arrière d'une charnière. Le braquer vers le bas de quelques degrés cambre le profil : la portance à un angle donné augmente (environ +0,05 à +0,1 de Cl par degré pour un volet de 25 % de corde), l'angle de portance nulle devient plus négatif et Cm plus négatif. Braqué vers le haut, il fait l'inverse (profil relevé).\n\nLa charnière est donnée par sa position x et soit son ordonnée y, soit sa hauteur en fraction de l'épaisseur locale (0 sur l'intrados, 1 sur l'extrados). Comme dans XFOIL, la surface où le volet rentre est coupée et un arc de cercle ferme l'ouverture de l'autre côté ; l'arrondi d'une vraie charnière et l'écoulement dans la fente ne sont pas représentés.",
      },
    },
  },
  {
    id: "modify",
    category: "geometry",
    text: {
      en: {
        term: "Modifying an airfoil",
        short: "XFOIL's geometry tools: flap, thickness and camber, their high points, trailing-edge gap, leading-edge radius, blending.",
        long: "Each tool creates a new airfoil in the library and keeps the original, so both can be compared. Thickness and camber can be scaled or set (camber line and thickness are measured vertically, as XFOIL does); the high points slide the maximum thickness or camber along the chord; the trailing-edge gap opens or closes the trailing edge over a blending distance; the leading-edge radius is scaled near the nose; blending interpolates between two airfoils point by point (a fraction outside 0–1 extrapolates). They are ports of XFOIL's GDES commands (FLAP, TFAC, TSET, HIGH, TGAP, LERA) and of INTE, checked against XFOIL.",
      },
      fr: {
        term: "Modifier un profil",
        short: "Les outils géométriques de XFOIL : volet, épaisseur et cambrure, leurs points hauts, épaisseur de bord de fuite, rayon de bord d'attaque, mélange.",
        long: "Chaque outil crée un nouveau profil dans la bibliothèque et garde l'original, pour pouvoir comparer les deux. L'épaisseur et la cambrure peuvent être multipliées ou fixées (la ligne de cambrure et l'épaisseur sont mesurées verticalement, comme dans XFOIL) ; les points hauts font glisser le maximum d'épaisseur ou de cambrure le long de la corde ; l'épaisseur de bord de fuite ouvre ou ferme le bord de fuite sur une distance de raccord ; le rayon de bord d'attaque est modifié près du nez ; le mélange interpole deux profils point par point (une fraction hors de 0–1 extrapole). Ce sont des portages des commandes GDES de XFOIL (FLAP, TFAC, TSET, HIGH, TGAP, LERA) et de INTE, vérifiés contre XFOIL.",
      },
    },
  },
  {
    id: "inverse-design",
    category: "geometry",
    text: {
      en: {
        term: "Inverse design",
        short: "Instead of computing the flow around a given shape, the shape is computed from the surface speed (or pressure) you want.",
        long: "In an analysis the shape is known and the speed along the surface is the result; inverse design turns this around. You draw the speed distribution you want — for example a lower suction peak, a flatter upper surface for longer laminar flow, a gentler pressure recovery toward the trailing edge — and the airfoil that produces it is computed.\n\nThe mixed inverse (XFOIL's QDES) reshapes only the segment of the surface where the target was changed, keeping the rest; a few free parameters let the speed join the unchanged parts, so the reached speed is the drawn one plus a small smooth offset along the segment, and the speed elsewhere changes a little too. The full inverse (XFOIL's MDES) generates the whole airfoil by conformal mapping of a circle (Lighthill's method): the target is adjusted to the nearest one an airfoil can produce, with the trailing-edge gap and angle you set. Not every target makes a sensible airfoil: draw smooth changes, use Smooth or Filter, and check the new shape and its analysis.",
      },
      fr: {
        term: "Conception inverse",
        short: "Au lieu de calculer l'écoulement autour d'une forme donnée, on calcule la forme à partir de la vitesse (ou de la pression) voulue sur sa surface.",
        long: "Dans une analyse, la forme est connue et la vitesse le long de la surface est le résultat ; la conception inverse fait le chemin inverse. Tu dessines la répartition de vitesse que tu veux — par exemple un pic de dépression plus faible, un extrados plus plat pour un écoulement laminaire plus long, une recompression plus douce vers le bord de fuite — et le profil qui la produit est calculé.\n\nLa méthode mixte (QDES de XFOIL) ne remodèle que le segment de surface où la cible a changé et garde le reste ; quelques paramètres libres permettent à la vitesse de se raccorder aux parties inchangées, donc la vitesse obtenue est celle dessinée plus un petit décalage régulier le long du segment, et la vitesse ailleurs change un peu aussi. La méthode complète (MDES de XFOIL) génère tout le profil par transformation conforme d'un cercle (méthode de Lighthill) : la cible est ajustée à la plus proche qu'un profil puisse produire, avec l'épaisseur et l'angle de bord de fuite choisis. Toute cible ne donne pas un profil raisonnable : dessine des changements doux, utilise Smooth ou Filter, et vérifie la nouvelle forme et son analyse.",
      },
    },
  },
  {
    id: "target-speed",
    category: "geometry",
    text: {
      en: {
        term: "Surface speed and target speed",
        short: "q/V∞: the flow speed just outside the surface divided by the freestream speed; Cp = 1 − (q/V∞)².",
        long: "The surface speed is the speed of the inviscid flow along the airfoil surface, divided by the freestream speed V∞. It is another way of looking at the pressure: where the speed is high the pressure is low, Cp = 1 − (q/V∞)² at low Mach number. It is zero at the stagnation point near the leading edge, rises to a peak (the suction peak) on the upper surface, then decreases toward the trailing edge — that deceleration is the adverse pressure gradient that the boundary layer must survive.\n\nIn inverse design the target speed is the distribution you want. It is plotted for both surfaces against x; the upper surface usually has the higher speeds when the airfoil lifts.",
      },
      fr: {
        term: "Vitesse de surface et vitesse cible",
        short: "q/V∞ : la vitesse de l'écoulement juste au-dessus de la surface divisée par la vitesse de l'écoulement amont ; Cp = 1 − (q/V∞)².",
        long: "La vitesse de surface est la vitesse de l'écoulement non visqueux le long de la surface du profil, divisée par la vitesse amont V∞. C'est une autre façon de voir la pression : là où la vitesse est grande la pression est basse, Cp = 1 − (q/V∞)² à faible Mach. Elle est nulle au point d'arrêt près du bord d'attaque, monte jusqu'à un pic (le pic de dépression) sur l'extrados, puis diminue vers le bord de fuite — ce ralentissement est le gradient de pression adverse que la couche limite doit supporter.\n\nEn conception inverse, la vitesse cible est la répartition que tu veux. Elle est tracée pour les deux faces en fonction de x ; l'extrados a en général les vitesses les plus grandes quand le profil porte.",
      },
    },
  },
  {
    id: "paneling",
    category: "geometry",
    text: {
      en: {
        term: "Panels and paneling",
        short: "The airfoil contour is cut into straight segments (panels) on which the flow is solved; 160 is a good default.",
        long: "The panel method represents the surface by N nodes joined by straight panels. XFOIL's PANGEN routine places them along a spline of the contour, closer together where the curvature is high (leading edge) and at the trailing edge. More panels give more accurate results but take longer; 160 to 240 nodes are usual. Results should hardly change when you increase the count — if they do, refine.",
      },
      fr: {
        term: "Panneaux et maillage",
        short: "Le contour du profil est découpé en segments droits (panneaux) sur lesquels on calcule l'écoulement ; 160 est un bon défaut.",
        long: "La méthode de panneaux représente la surface par N nœuds reliés par des panneaux droits. La routine PANGEN de XFOIL les place le long d'une spline du contour, plus serrés là où la courbure est forte (bord d'attaque) et au bord de fuite. Plus de panneaux donnent des résultats plus précis mais plus lents ; 160 à 240 nœuds sont habituels. Les résultats ne devraient presque pas changer quand on en augmente le nombre — sinon, raffine.",
      },
    },
  },
  // ------------------------------------------------------- options
  {
    id: "prescribed-cl",
    category: "options",
    text: {
      en: {
        term: "Prescribed α or Cl",
        short: "Either you set the angle of attack and get Cl, or you set Cl and the solver finds the angle (XFOIL's CL command).",
        long: "An aircraft in flight needs a given lift (to balance its weight), not a given angle. Prescribing Cl compares airfoils fairly at the same lift: which one makes the least drag at Cl = 0.5? The solver then finds the angle of attack together with the boundary layer.",
      },
      fr: {
        term: "α ou Cl imposé",
        short: "Soit tu fixes l'angle d'attaque et tu obtiens Cl, soit tu fixes Cl et le solveur trouve l'angle (commande CL de XFOIL).",
        long: "Un avion en vol a besoin d'une portance donnée (pour équilibrer son poids), pas d'un angle donné. Imposer Cl permet de comparer des profils équitablement à portance égale : lequel traîne le moins à Cl = 0,5 ? Le solveur trouve alors l'angle d'attaque en même temps que la couche limite.",
      },
    },
  },
  {
    id: "mach-re-variation",
    symbol: "M, Re",
    category: "options",
    text: {
      en: {
        term: "Mach and Reynolds varying with Cl",
        short: "For a given aircraft, flying at a higher Cl means flying slower: M and Re then decrease (XFOIL TYPE 2 and 3).",
        long: "With “fixed”, Re and M are the same at every point. With “∝ 1/√Cl” (XFOIL TYPE 2), they vary as for an aircraft of fixed weight and wing: the speed, hence M and Re, is proportional to 1/√Cl, and the values you enter are those at Cl = 1. With “Re ∝ 1/Cl” (TYPE 3), Re varies as 1/Cl (fixed lift, varying chord). This gives realistic polars for a given aircraft.",
      },
      fr: {
        term: "Mach et Reynolds variant avec Cl",
        short: "Pour un avion donné, voler à un Cl plus grand veut dire voler plus lentement : M et Re diminuent alors (TYPE 2 et 3 de XFOIL).",
        long: "Avec « fixed », Re et M sont les mêmes à chaque point. Avec « ∝ 1/√Cl » (TYPE 2 de XFOIL), ils varient comme pour un avion de masse et d'aile fixées : la vitesse, donc M et Re, est proportionnelle à 1/√Cl, et les valeurs saisies sont celles à Cl = 1. Avec « Re ∝ 1/Cl » (TYPE 3), Re varie comme 1/Cl (portance fixée, corde variable). Cela donne des polaires réalistes pour un avion donné.",
      },
    },
  },
  {
    id: "amplification",
    symbol: "Ampl.",
    category: "options",
    text: {
      en: {
        term: "Amplification model",
        short: "Which correlation of disturbance growth is used for the transition: XFOIL's original one or the modified one of its DAMP command.",
        long: "Both are envelope correlations of the eᴺ method. The original one is XFOIL's default and the one used for its validation; the modified one (DAMP) changes the growth rates of the disturbances in some boundary-layer states. Keep “original” unless you are comparing with results obtained with DAMP.",
      },
      fr: {
        term: "Modèle d'amplification",
        short: "Quelle corrélation de croissance des perturbations est utilisée pour la transition : celle d'origine de XFOIL ou la version modifiée de sa commande DAMP.",
        long: "Ce sont deux corrélations d'enveloppe de la méthode eᴺ. Celle d'origine est la valeur par défaut de XFOIL et celle de sa validation ; la version modifiée (DAMP) change les taux de croissance des perturbations dans certains états de couche limite. Garde « original » sauf pour comparer à des résultats obtenus avec DAMP.",
      },
    },
  },
  {
    id: "convergence",
    category: "options",
    text: {
      en: {
        term: "Convergence",
        short: "The viscous solution is found by successive corrections; it has converged when the corrections become negligible.",
        long: "The boundary layer and the outer flow are solved together by Newton iterations (20 at most for one point, as in XFOIL). Near the stall, or when the transition hesitates between two positions, the iterations may not settle: the point is then reported as not converged and should not be trusted. Approaching it from a neighbouring point (a sweep) often helps.",
      },
      fr: {
        term: "Convergence",
        short: "La solution visqueuse est trouvée par corrections successives ; elle a convergé quand les corrections deviennent négligeables.",
        long: "La couche limite et l'écoulement extérieur sont résolus ensemble par des itérations de Newton (20 au plus pour un point, comme dans XFOIL). Près du décrochage, ou quand la transition hésite entre deux positions, les itérations peuvent ne pas se stabiliser : le point est alors signalé non convergé et ne doit pas être utilisé. L'approcher depuis un point voisin (balayage) aide souvent.",
      },
    },
  },
  // -------------------------------------------------- optimization
  {
    id: "optimization",
    category: "optimization",
    text: {
      en: {
        term: "Shape optimization",
        short: "The computer reshapes the airfoil, trying many candidates, to improve an objective while respecting constraints.",
        long: "You choose operating points (Cl or α, each with a weight), an objective (least drag, or best L/D) and constraints (thickness, moment…). The optimizer then analyzes hundreds of slightly different shapes with the viscous solver and keeps improving the best one.\n\nAn airfoil optimized for one point is often worse elsewhere: give several points, or check the polars afterwards (“Compare polars”).",
      },
      fr: {
        term: "Optimisation de forme",
        short: "L'ordinateur remodèle le profil en essayant de nombreux candidats, pour améliorer un objectif en respectant des contraintes.",
        long: "Tu choisis des points de fonctionnement (Cl ou α, chacun avec un poids), un objectif (le moins de traînée, ou la meilleure finesse) et des contraintes (épaisseur, moment…). L'optimiseur analyse alors des centaines de formes légèrement différentes avec le solveur visqueux et améliore sans cesse la meilleure.\n\nUn profil optimisé pour un seul point est souvent moins bon ailleurs : donne plusieurs points, ou vérifie les polaires ensuite (« Compare polars »).",
      },
    },
  },
  {
    id: "cst",
    symbol: "CST",
    category: "optimization",
    text: {
      en: {
        term: "CST parameterization",
        short: "A way to describe a smooth airfoil with a few numbers (weights of polynomials), which the optimizer changes.",
        long: "The Class/Shape Transformation (Kulfan) writes each surface as a round-nose, sharp-tail “class” shape multiplied by a smooth polynomial. With order 6, each surface has 7 weights: 14 numbers describe the whole airfoil. They are first fitted to your airfoil; the fit error is shown. Higher orders allow finer shapes but make the search longer.",
      },
      fr: {
        term: "Paramétrisation CST",
        short: "Une façon de décrire un profil lisse avec quelques nombres (poids de polynômes), que l'optimiseur fait varier.",
        long: "La Class/Shape Transformation (Kulfan) écrit chaque face comme une forme de « classe » à nez rond et queue pointue multipliée par un polynôme lisse. À l'ordre 6, chaque face a 7 poids : 14 nombres décrivent tout le profil. Ils sont d'abord ajustés sur ton profil ; l'erreur d'ajustement est affichée. Un ordre plus élevé permet des formes plus fines mais allonge la recherche.",
      },
    },
  },
  {
    id: "objective",
    category: "optimization",
    text: {
      en: {
        term: "Objective",
        short: "What the optimizer tries to improve: the weighted mean drag, or the weighted mean L/D, over the operating points.",
        long: "With several operating points, each has a weight: a point with weight 2 counts twice as much. A typical choice for a sailplane is a low Cl (fast cruise) and a high Cl (thermalling), with weights reflecting the time spent in each.",
      },
      fr: {
        term: "Objectif",
        short: "Ce que l'optimiseur cherche à améliorer : la traînée moyenne pondérée, ou la finesse moyenne pondérée, sur les points de fonctionnement.",
        long: "Avec plusieurs points de fonctionnement, chacun a un poids : un point de poids 2 compte deux fois plus. Un choix typique pour un planeur est un Cl faible (croisière rapide) et un Cl élevé (spirale en thermique), avec des poids qui reflètent le temps passé dans chacun.",
      },
    },
  },
  {
    id: "constraint",
    category: "optimization",
    text: {
      en: {
        term: "Constraints and feasible designs",
        short: "Conditions the result must satisfy (minimum thickness…); a design that violates one is “infeasible”.",
        long: "Without constraints, an optimizer reducing drag would make the airfoil paper-thin. Constraints keep it buildable: minimum thickness (overall or at a spar station), minimum Cm, minimum trailing-edge angle. A design that violates a constraint, or whose analysis does not converge, is infeasible; a feasible design always beats an infeasible one, so the result satisfies the constraints whenever possible.",
      },
      fr: {
        term: "Contraintes et formes admissibles",
        short: "Des conditions que le résultat doit respecter (épaisseur minimale…) ; une forme qui en viole une est « non admissible ».",
        long: "Sans contraintes, un optimiseur qui réduit la traînée rendrait le profil aussi fin qu'une feuille. Les contraintes le gardent réalisable : épaisseur minimale (globale ou à l'emplacement d'un longeron), Cm minimal, angle de bord de fuite minimal. Une forme qui viole une contrainte, ou dont l'analyse ne converge pas, est non admissible ; une forme admissible bat toujours une forme non admissible, donc le résultat respecte les contraintes dès que c'est possible.",
      },
    },
  },
  {
    id: "optimizer",
    category: "optimization",
    text: {
      en: {
        term: "CMA-ES and Nelder–Mead",
        short: "The two search methods: CMA-ES explores widely with populations of candidates, Nelder–Mead refines the best one.",
        long: "CMA-ES (covariance matrix adaptation evolution strategy) draws populations of random candidates around the current best, and learns which directions pay off; each population is analyzed in parallel on all cores. Nelder–Mead then polishes the result by moving a small simplex of candidates. The seed fixes the random draws: the same seed gives exactly the same result.",
      },
      fr: {
        term: "CMA-ES et Nelder–Mead",
        short: "Les deux méthodes de recherche : CMA-ES explore largement avec des populations de candidats, Nelder–Mead affine le meilleur.",
        long: "CMA-ES (stratégie d'évolution par adaptation de la matrice de covariance) tire des populations de candidats aléatoires autour du meilleur actuel, et apprend quelles directions sont payantes ; chaque population est analysée en parallèle sur tous les cœurs. Nelder–Mead peaufine ensuite le résultat en déplaçant un petit simplexe de candidats. La graine (seed) fixe les tirages aléatoires : la même graine donne exactement le même résultat.",
      },
    },
  },
  // ---------------------------------------------------------- aircraft
  {
    id: "wing-loading",
    category: "aircraft",
    text: {
      en: {
        term: "Wing loading",
        short: "The aircraft's weight divided by its wing area: what each square decimetre of wing carries.",
        long: "Wing loading = weight ÷ wing area, shown in grams per square decimetre (g/dm²), the unit of model aircraft (1 kg/m² = 10 g/dm²). A light wing loading makes a wing that flies slowly and gently; a heavy one needs speed to take off and land, but goes through wind and turbulence better.\n\nWith the same airfoil, the stall speed grows with the square root of the wing loading: doubling the loading raises it by about 41 %. Typical values: indoor models under 15 g/dm², park flyers and gliders 15 to 35, sport models 35 to 70, a Cessna 172 about 690.",
      },
      fr: {
        term: "Charge alaire",
        short: "Le poids de l'appareil divisé par la surface de l'aile : ce que porte chaque décimètre carré d'aile.",
        long: "Charge alaire = poids ÷ surface alaire, affichée en grammes par décimètre carré (g/dm²), l'unité des modélistes (1 kg/m² = 10 g/dm²). Une charge alaire faible donne une aile qui vole lentement et en douceur ; une charge élevée demande de la vitesse pour décoller et se poser, mais passe mieux le vent et la turbulence.\n\nAvec le même profil, la vitesse de décrochage croît comme la racine carrée de la charge alaire : doubler la charge l'augmente d'environ 41 %. Valeurs typiques : modèles d'intérieur sous 15 g/dm², park flyers et planeurs 15 à 35, modèles de sport 35 à 70, un Cessna 172 environ 690.",
      },
    },
  },
  {
    id: "atmosphere",
    category: "aircraft",
    text: {
      en: {
        term: "Standard atmosphere",
        short: "The model of the air used in aviation: temperature, pressure and density as a function of altitude, on an average day.",
        long: "The International Standard Atmosphere (ISA) describes an average day: 15 °C and 1013.25 hPa at sea level, the temperature falling by 6.5 °C per kilometre up to 11 km, then constant at −56.5 °C. Aero computes the density and the viscosity of the air from it (U.S. Standard Atmosphere 1976, checked against its tables).\n\nThe thinner the air, the faster a wing must fly to make the same lift: at 3000 m the density is 74 % of its sea-level value. A hot day thins the air too: set the temperature offset in the advanced settings of My aircraft.",
      },
      fr: {
        term: "Atmosphère standard",
        short: "Le modèle de l'air utilisé en aviation : température, pression et masse volumique en fonction de l'altitude, pour une journée moyenne.",
        long: "L'atmosphère type internationale (ISA) décrit une journée moyenne : 15 °C et 1013,25 hPa au niveau de la mer, une température qui baisse de 6,5 °C par kilomètre jusqu'à 11 km, puis reste à −56,5 °C. Aero en déduit la masse volumique et la viscosité de l'air (U.S. Standard Atmosphere 1976, vérifiée contre ses tables).\n\nPlus l'air est léger, plus une aile doit voler vite pour produire la même portance : à 3000 m la masse volumique vaut 74 % de sa valeur au niveau de la mer. Une journée chaude allège aussi l'air : règle l'écart de température dans les réglages avancés de Mon avion.",
      },
    },
  },
  {
    id: "aspect-ratio",
    category: "aircraft",
    text: {
      en: {
        term: "Aspect ratio",
        short: "How slender a wing is: span squared divided by wing area (span ÷ chord for a rectangular wing).",
        long: "A long, narrow wing (a high aspect ratio, 10 to 30 for sailplanes) loses less energy in its wing-tip vortices, so it has less induced drag. A short, wide wing (4 to 7 for trainers and aerobatic models) is sturdier and turns faster. Aero uses the aspect ratio in the induced-drag estimate of My aircraft.",
      },
      fr: {
        term: "Allongement",
        short: "La finesse de forme d'une aile : l'envergure au carré divisée par la surface (envergure ÷ corde pour une aile rectangulaire).",
        long: "Une aile longue et étroite (grand allongement, 10 à 30 pour les planeurs) perd moins d'énergie dans ses tourbillons marginaux, donc a moins de traînée induite. Une aile courte et large (4 à 7 pour les avions d'école et de voltige) est plus solide et tourne plus vite. Aero utilise l'allongement dans l'estimation de la traînée induite de Mon avion.",
      },
    },
  },
  {
    id: "induced-drag",
    category: "aircraft",
    text: {
      en: {
        term: "Induced drag",
        short: "The drag a finite wing pays for making lift: air spills around the tips and forms vortices.",
        long: "Air at high pressure under the wing flows around the tips towards the low pressure above, forming wing-tip vortices; the wing pays for them with extra drag. The classical estimate is CDi = CL² ÷ (π e AR): it grows with the square of the lift coefficient and falls with the aspect ratio AR; the span efficiency e (about 0.9 for a straight rectangular wing, 1 for an elliptic one) measures how well the lift is spread along the span.\n\nAt low speed, when CL is high, induced drag is often the largest part of a model's drag. A 2D airfoil analysis does not include it: Aero adds it in the wing estimate of My aircraft.",
      },
      fr: {
        term: "Traînée induite",
        short: "La traînée qu'une aile de longueur finie paie pour porter : l'air contourne les bouts d'aile et forme des tourbillons.",
        long: "L'air en surpression sous l'aile contourne les saumons vers la dépression du dessus, ce qui forme les tourbillons marginaux ; l'aile les paie par une traînée supplémentaire. L'estimation classique est CDi = CL² ÷ (π e AR) : elle croît comme le carré du coefficient de portance et diminue avec l'allongement AR ; le coefficient d'Oswald e (environ 0,9 pour une aile rectangulaire droite, 1 pour une aile elliptique) mesure la qualité de la répartition de la portance le long de l'envergure.\n\nÀ basse vitesse, quand CL est grand, la traînée induite est souvent la plus grosse part de la traînée d'un modèle. Une analyse de profil 2D ne la compte pas : Aero l'ajoute dans l'estimation de l'aile de Mon avion.",
      },
    },
  },
  {
    id: "stall-speed",
    category: "aircraft",
    text: {
      en: {
        term: "Stall speed",
        short: "The slowest speed at which the wing still carries the aircraft in level flight.",
        long: "Below this speed the wing would need more than its maximum lift coefficient: it stalls. V = √(2 W ÷ (ρ S CLmax)), with W the weight, ρ the air density, S the wing area and CLmax the wing's maximum lift coefficient. Aero takes 90 % of the airfoil's: a whole wing stalls a little before its section, and XFOIL-type methods over-predict the maximum lift by about that much (10 % on the E387 compared with NASA wind-tunnel measurements).\n\nFly comfortably above it: 1.2 to 1.3 times the stall speed is a usual margin.",
      },
      fr: {
        term: "Vitesse de décrochage",
        short: "La vitesse la plus faible à laquelle l'aile porte encore l'appareil en vol en palier.",
        long: "En dessous de cette vitesse, l'aile aurait besoin de plus que son coefficient de portance maximal : elle décroche. V = √(2 W ÷ (ρ S CLmax)), avec W le poids, ρ la masse volumique de l'air, S la surface alaire et CLmax le coefficient de portance maximal de l'aile. Aero prend 90 % de celui du profil : une aile entière décroche un peu avant sa section, et les méthodes de type XFOIL surestiment la portance maximale d'à peu près autant (10 % sur l'E387 comparé aux mesures en soufflerie de la NASA).\n\nVole confortablement au-dessus : 1,2 à 1,3 fois la vitesse de décrochage est une marge habituelle.",
      },
    },
  },
  {
    id: "tail-volume",
    category: "aircraft",
    text: {
      en: {
        term: "Tail volume",
        short: "How big the tail is for the wing it balances: tail area times its distance from the wing, over the wing's area and chord.",
        long: "The horizontal tail volume is Vh = Sh·lh ÷ (S·c̄): the tail area Sh times its distance lh behind the wing (between the quarter points of their mean chords), divided by the wing area S times its mean chord c̄. A small tail far behind does the same job as a large tail close by. Usual values are 0.35 to 0.8: below about 0.3 the aircraft tends to be twitchy in pitch.\n\nThe fin's volume is Vv = Sv·lv ÷ (S·b), with the wing span b; usual values are 0.02 to 0.06. These are rules of thumb from existing aircraft; the 3D analysis (\"How it flies\") gives the real stability.",
      },
      fr: {
        term: "Volume d'empennage",
        short: "La taille de l'empennage par rapport à l'aile qu'il équilibre : sa surface fois sa distance à l'aile, rapportées à la surface et à la corde de l'aile.",
        long: "Le volume d'empennage horizontal vaut Vh = Sh·lh ÷ (S·c̄) : la surface Sh de l'empennage fois sa distance lh derrière l'aile (entre les quarts avant de leurs cordes moyennes), divisées par la surface S de l'aile fois sa corde moyenne c̄. Un petit empennage loin derrière fait le même travail qu'un grand empennage proche. Les valeurs habituelles vont de 0,35 à 0,8 : en dessous de 0,3 environ, l'avion a tendance à être nerveux en tangage.\n\nLe volume de dérive vaut Vv = Sv·lv ÷ (S·b), avec b l'envergure de l'aile ; valeurs habituelles 0,02 à 0,06. Ce sont des règles tirées des avions existants ; l'analyse 3D (« Comment il vole ») donne la stabilité réelle.",
      },
    },
  },
  {
    id: "trim",
    category: "aircraft",
    text: {
      en: {
        term: "Trim",
        short: "The flight where the aircraft balances itself: lift equal to the weight and no pitching moment about the centre of gravity.",
        long: "An aircraft is trimmed when it flies steadily without the pilot holding the stick: the wing and the tail together lift exactly the weight, and their pitching moments about the centre of gravity cancel. The angle of attack and the elevator deflection that do this depend on the speed and on where the centre of gravity is.\n\nThe 3D analysis finds them by solving both conditions together. If it needs more than about 15° of elevator, the aircraft is badly balanced: the centre of gravity is too far forward or back, or the tail is too small.",
      },
      fr: {
        term: "Équilibre (trim)",
        short: "Le vol où l'avion s'équilibre seul : portance égale au poids et aucun moment de tangage autour du centre de gravité.",
        long: "Un avion est équilibré quand il vole sans que le pilote tienne le manche : l'aile et l'empennage portent ensemble exactement le poids, et leurs moments de tangage autour du centre de gravité s'annulent. L'incidence et le braquage de profondeur qui y parviennent dépendent de la vitesse et de la position du centre de gravité.\n\nL'analyse 3D les trouve en résolvant les deux conditions ensemble. S'il faut plus de 15° de profondeur environ, l'avion est mal centré : centre de gravité trop en avant ou trop en arrière, ou empennage trop petit.",
      },
    },
  },
  {
    id: "static-margin",
    category: "aircraft",
    text: {
      en: {
        term: "Static margin, neutral point",
        short: "How far the centre of gravity is ahead of the neutral point, as a fraction of the mean chord: positive is stable, 5 to 15 % is usual.",
        long: "The neutral point is where the extra lift of a gust acts on the whole aircraft: with the centre of gravity there, a nose-up gust neither pitches the nose further up nor brings it back. With the centre of gravity ahead of it, the aircraft weathervanes back to its trimmed angle: it is stable in pitch.\n\nThe static margin is the distance from the centre of gravity forward to the neutral point, divided by the wing's mean aerodynamic chord. Below zero the aircraft is unstable and hard or impossible to fly; a few percent makes it twitchy; 5 to 15 % is usual for models; much more makes it nose-heavy, needing more elevator and flying slower than it could. The fuselage, not part of the vortex lattice, usually moves the neutral point forward by a few percent.",
      },
      fr: {
        term: "Marge statique, foyer",
        short: "La distance dont le centre de gravité est en avant du foyer, rapportée à la corde moyenne : positive c'est stable, 5 à 15 % est habituel.",
        long: "Le foyer (point neutre) est l'endroit où s'applique le surcroît de portance d'une rafale sur tout l'avion : avec le centre de gravité à cet endroit, une rafale qui lève le nez ne le fait ni monter davantage ni revenir. Avec le centre de gravité en avant du foyer, l'avion revient de lui-même à son incidence d'équilibre : il est stable en tangage.\n\nLa marge statique est la distance du centre de gravité au foyer, vers l'avant, divisée par la corde aérodynamique moyenne de l'aile. En dessous de zéro l'avion est instable, difficile voire impossible à piloter ; quelques pour cent le rendent vif ; 5 à 15 % est habituel pour les modèles ; beaucoup plus le rend lourd de l'avant : il demande plus de profondeur et vole moins lentement qu'il pourrait. Le fuselage, absent du treillis de tourbillons, avance en général le foyer de quelques pour cent.",
      },
    },
  },
  {
    id: "vortex-lattice",
    category: "aircraft",
    text: {
      en: {
        term: "Vortex lattice",
        short: "The 3D method of AVL: each lifting surface becomes a grid of small horseshoe vortices whose strengths make the flow follow the surfaces.",
        long: "The vortex lattice method models the wing, the tail and the fin as thin surfaces covered with horseshoe vortices: a bound segment across each small panel and two trailing legs running downstream. Their strengths are chosen so that the air flows along the surfaces (along their camber lines) at one control point per panel. The lift, the pitching and yawing moments, the stability derivatives and the induced drag follow, quickly and accurately for attached flow.\n\nIt does not know about viscosity: the friction drag and the maximum lift come from the 2D viscous polars of the airfoils, strip by strip, as in XFLR5. Aero's lattice is a port of AVL 3.52 (Mark Drela and Harold Youngren, MIT), checked against AVL itself.",
      },
      fr: {
        term: "Treillis de tourbillons (VLM)",
        short: "La méthode 3D d'AVL : chaque surface portante devient une grille de petits tourbillons en fer à cheval dont les intensités font suivre les surfaces à l'écoulement.",
        long: "La méthode des tourbillons en treillis modélise l'aile, l'empennage et la dérive comme des surfaces minces couvertes de tourbillons en fer à cheval : un segment lié en travers de chaque petit panneau et deux brins libres qui partent vers l'aval. Leurs intensités sont choisies pour que l'air longe les surfaces (leurs lignes moyennes) en un point de contrôle par panneau. On en tire la portance, les moments de tangage et de lacet, les dérivées de stabilité et la traînée induite, rapidement et précisément tant que l'écoulement reste attaché.\n\nElle ignore la viscosité : la traînée de frottement et la portance maximale viennent des polaires visqueuses 2D des profils, bande par bande, comme dans XFLR5. Le treillis d'Aero est un portage d'AVL 3.52 (Mark Drela et Harold Youngren, MIT), vérifié face à AVL lui-même.",
      },
    },
  },
];

/** The entry with this id. */
export function glossaryEntry(id: string): GlossaryEntry | undefined {
  return GLOSSARY.find((e) => e.id === id);
}

/** Entries whose symbol, term or text contains `query` (case-insensitive). */
export function searchGlossary(query: string, lang: TourLang): GlossaryEntry[] {
  const q = query.trim().toLowerCase();
  if (!q) return GLOSSARY;
  return GLOSSARY.filter((e) => {
    const t = e.text[lang];
    return [e.symbol ?? "", t.term, t.short, t.long].some((s) => s.toLowerCase().includes(q));
  });
}

const DOC_INTRO: Record<TourLang, string> = {
  en: "# Glossary\n\nThe quantities and terms of the Aero application, explained for non-specialists. The same text is shown in the application (Glossary button, and on the labels with a dotted underline).\n\n_Generated from `app/src/lib/glossary.ts` by `npm run glossary`; do not edit by hand. Version française : [glossaire.md](glossaire.md)._\n",
  fr: "# Glossaire\n\nLes grandeurs et les termes de l'application Aero, expliqués pour les non-spécialistes. Le même texte est affiché dans l'application (bouton Glossary, et sur les libellés soulignés en pointillés).\n\n_Généré depuis `app/src/lib/glossary.ts` par `npm run glossary` ; ne pas modifier à la main. English version: [glossary.md](glossary.md)._\n",
};

/** The glossary as a Markdown document. */
export function glossaryMarkdown(lang: TourLang): string {
  const out = [DOC_INTRO[lang]];
  for (const category of Object.keys(CATEGORY_TITLES) as GlossaryCategory[]) {
    const entries = GLOSSARY.filter((e) => e.category === category);
    if (entries.length === 0) continue;
    out.push(`## ${CATEGORY_TITLES[category][lang]}\n`);
    for (const e of entries) {
      const t = e.text[lang];
      const symbol = e.symbol ? ` (${e.symbol})` : "";
      out.push(`### ${t.term}${symbol}\n\n**${t.short}**\n\n${t.long}\n`);
    }
  }
  return out.join("\n");
}

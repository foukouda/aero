// Interface texts in English and French. Components call `app.t(key)`;
// `{name}` placeholders are replaced by the given parameters. Both
// dictionaries must have the same keys and placeholders (i18n.test.ts).

import type { TourLang } from "./tour";

export type Lang = TourLang;

const en = {
  // Tabs.
  "tab.mission": "My aircraft",
  "tab.geometry": "Geometry",
  "tab.analysis": "Analysis",
  "tab.polars": "Polars",
  "tab.optimize": "Optimize",
  "tab.design": "Inverse design",

  // My aircraft: the flight.
  "mission.title": "Describe your aircraft",
  "mission.intro":
    "Start from the flight you have in mind. Aero works out what the wing has to do and which airfoils suit it.",
  "mission.presets": "Examples",
  "preset.parkflyer": "Slow park flyer",
  "preset.glider": "1.5 m glider",
  "preset.trainer": "Foam trainer",
  "preset.drone": "2 kg drone",
  "preset.cessna": "Light aircraft (Cessna 172)",
  "mission.speed": "I want to fly at",
  "mission.altitude": "at an altitude of",
  "mission.mass": "with an aircraft weighing",
  "mission.wing": "The wing",
  "mission.wingKnown": "I know my wing",
  "mission.wingSized": "Size it for me",
  "mission.span": "Span, tip to tip",
  "mission.chord": "Chord, front to back",
  "mission.sizingCl": "Fly at a lift coefficient of",
  "mission.sizingHint": "Between 0.5 and 0.7 suits most airfoils.",
  "mission.sizedWing": "Chord {chord}, area {area}",
  "mission.goal": "What matters most?",
  "goal.least_drag": "Fly efficiently",
  "goal.least_drag.hint": "The least drag at your speed: the longest glide, the longest flight on a battery.",
  "goal.slow_flight": "Fly slowly, carry more",
  "goal.slow_flight.hint": "The most lift before the stall.",
  "goal.easy_to_build": "Easy to build",
  "goal.easy_to_build.hint": "Thick enough and no hollow underside; then the least drag.",
  "mission.advanced": "Advanced settings",
  "mission.air": "Air",
  "turbulence.calm": "Calm (Ncrit 9)",
  "turbulence.normal": "A little turbulent (Ncrit 7)",
  "turbulence.rough": "Turbulent (Ncrit 5)",
  "mission.temperature": "Temperature offset, °C",
  "mission.temperatureHint": "0 for a standard day (15 °C at sea level), +15 for a hot day.",
  "mission.spanEfficiency": "Span efficiency e",
  "mission.clMaxFactor": "Wing Cl max ÷ airfoil Cl max",
  "mission.includeLibrary": "Also compare the airfoils of my library ({count})",
  "mission.find": "Find airfoils",
  "mission.finding": "Analysing {done} of {total}…",
  "mission.stop": "Stop",
  "mission.flightError": "This flight cannot be computed: {message}",

  // What this means.
  "card.title": "What this means",
  "card.cl": "Lift coefficient needed",
  "card.cl.low": "Low: the wing is larger than needed at this speed. It flies, but a smaller wing would have less drag.",
  "card.cl.ok": "Comfortable: most airfoils fly here with a good margin before the stall.",
  "card.cl.high": "High: few airfoils keep a safe margin. A larger wing or a little more speed would help.",
  "card.cl.veryHigh":
    "Very high: only high-lift airfoils manage this, close to the stall. Make the wing larger or fly faster.",
  "card.re": "Reynolds number",
  "card.re.veryLow":
    "Very low: a tiny or very slow wing. The flow stays laminar and separates easily; predictions are uncertain.",
  "card.re.low":
    "Low, typical of small slow models: laminar separation bubbles matter, and results are less reliable below 100 000.",
  "card.re.model": "The range of model aircraft and drones.",
  "card.re.large": "Large models, sailplanes and ultralights.",
  "card.re.full": "Full-size aircraft.",
  "card.loading": "Wing loading",
  "loading.veryLight": "Very light, like an indoor or free-flight model (under 15 g/dm²).",
  "loading.light": "Light, like a park flyer or a glider (15 to 35 g/dm²).",
  "loading.medium": "Medium, like a sport model or a trainer (35 to 70 g/dm²).",
  "loading.heavy": "Heavy for a model: it flies and lands fast (70 to 150 g/dm²).",
  "loading.fullSize": "In the range of full-size aircraft (a Cessna 172 carries about 690 g/dm²).",
  "card.air": "Air at {altitude}",
  "card.air.text": "{temperature}; the density is {percent} of its sea-level value.",
  "card.mach": "Mach number",
  "card.mach.ok": "Low enough for the air to be treated as incompressible.",
  "card.mach.high": "Compressibility matters above 0.3: Aero corrects for it, up to the critical Mach number.",
  "sketch.title": "Your wing, to scale",
  "sketch.a4": "A4 sheet",
  "sketch.person": "Person, 1.75 m",
  "level.ok": "OK",
  "level.info": "Note",
  "level.warning": "Watch out",
  "level.danger": "Problem",

  // Recommendations.
  "results.title": "Recommended airfoils",
  "results.empty":
    "No recommendation yet. Describe your flight, then press “Find airfoils”: Aero analyses the {count} airfoils of its catalogue at your Reynolds number, in a few seconds.",
  "results.context": "For Cl {cl} at Re {re}, sorted by: {goal}",
  "results.none":
    "No airfoil flies this with a safe margin before the stall. Make the wing larger, fly faster or make the aircraft lighter.",
  "results.lowRe":
    "Below Re 100 000 predictions are less reliable: compare a few airfoils rather than trusting a single number.",
  "results.estimate":
    "Wing only, simple finite-wing formula: the drag of the fuselage, tail and propeller is not included.",
  "fit.comfortable": "Comfortable margin",
  "fit.tight": "Tight margin",
  "fit.near_stall": "Too close to the stall",
  "fit.cannot_fly": "Cannot carry the aircraft",
  "fit.no_data": "Not reached at this lift",
  "cand.stall": "Stalls at",
  "cand.glide": "Glide ratio",
  "cand.power": "Wing power",
  "cand.angle": "Angle of attack",
  "cand.cd": "Drag coefficient",
  "cand.clmax": "Maximum lift",
  "cand.thickness": "Thickness",
  "cand.easy": "Easy to build",
  "cand.hard": "Hollow underside: harder to build",
  "cand.thin": "Thin: harder to build",
  "cand.fromLibrary": "From your library",
  "cand.analyse": "Analyse it",
  "cand.optimize": "Optimize it for my flight",
  "cand.failed": "Could not be analysed: {message}",
  "table.title": "All airfoils",
  "table.airfoil": "Airfoil",
  "table.fit": "Margin",
  "table.build": "Build",
  "polar.title": "Drag polars at your Reynolds number",
  "polar.required": "Cl needed",
  "toast.analyse.title": "Ready to analyse",
  "toast.analyse": "{name} is in your library and the Analysis tab is set to your flight (Re {re}, Cl {cl}).",
  "toast.optimize.title": "Ready to optimize",
  "toast.optimize":
    "{name} is in your library. The optimization looks for the least drag at Cl {cl} and Re {re}, keeping at least its thickness.",
} as const;

export type MessageKey = keyof typeof en;

const fr: Record<MessageKey, string> = {
  "tab.mission": "Mon avion",
  "tab.geometry": "Géométrie",
  "tab.analysis": "Analyse",
  "tab.polars": "Polaires",
  "tab.optimize": "Optimiser",
  "tab.design": "Conception inverse",

  "mission.title": "Décris ton avion",
  "mission.intro":
    "Pars du vol que tu as en tête. Aero calcule ce que l'aile doit faire et quels profils lui conviennent.",
  "mission.presets": "Exemples",
  "preset.parkflyer": "Park flyer lent",
  "preset.glider": "Planeur de 1,5 m",
  "preset.trainer": "Avion d'école en mousse",
  "preset.drone": "Drone de 2 kg",
  "preset.cessna": "Avion léger (Cessna 172)",
  "mission.speed": "Je veux voler à",
  "mission.altitude": "à une altitude de",
  "mission.mass": "avec un appareil de",
  "mission.wing": "L'aile",
  "mission.wingKnown": "Je connais mon aile",
  "mission.wingSized": "Dimensionne-la pour moi",
  "mission.span": "Envergure, d'un bout à l'autre",
  "mission.chord": "Corde, de l'avant à l'arrière",
  "mission.sizingCl": "Voler à un coefficient de portance de",
  "mission.sizingHint": "Entre 0,5 et 0,7, la plupart des profils sont à l'aise.",
  "mission.sizedWing": "Corde {chord}, surface {area}",
  "mission.goal": "Qu'est-ce qui compte le plus ?",
  "goal.least_drag": "Voler efficacement",
  "goal.least_drag.hint": "Le moins de traînée à ta vitesse : le plus long plané, le plus long vol sur batterie.",
  "goal.slow_flight": "Voler lentement, porter plus",
  "goal.slow_flight.hint": "Le plus de portance avant le décrochage.",
  "goal.easy_to_build": "Facile à construire",
  "goal.easy_to_build.hint": "Assez épais et sans creux sous l'aile ; puis le moins de traînée.",
  "mission.advanced": "Réglages avancés",
  "mission.air": "Air",
  "turbulence.calm": "Calme (Ncrit 9)",
  "turbulence.normal": "Un peu turbulent (Ncrit 7)",
  "turbulence.rough": "Turbulent (Ncrit 5)",
  "mission.temperature": "Écart de température, °C",
  "mission.temperatureHint": "0 pour une journée standard (15 °C au niveau de la mer), +15 pour une journée chaude.",
  "mission.spanEfficiency": "Coefficient d'Oswald e",
  "mission.clMaxFactor": "Cl max de l'aile ÷ Cl max du profil",
  "mission.includeLibrary": "Comparer aussi les profils de ma bibliothèque ({count})",
  "mission.find": "Trouver des profils",
  "mission.finding": "Analyse {done} sur {total}…",
  "mission.stop": "Arrêter",
  "mission.flightError": "Ce vol ne peut pas être calculé : {message}",

  "card.title": "Ce que ça veut dire",
  "card.cl": "Coefficient de portance nécessaire",
  "card.cl.low":
    "Faible : l'aile est plus grande que nécessaire à cette vitesse. Ça vole, mais une aile plus petite traînerait moins.",
  "card.cl.ok": "Confortable : la plupart des profils volent ici avec une bonne marge avant le décrochage.",
  "card.cl.high": "Élevé : peu de profils gardent une marge sûre. Une aile plus grande ou un peu plus de vitesse aiderait.",
  "card.cl.veryHigh":
    "Très élevé : seuls des profils hypersustentateurs y arrivent, près du décrochage. Agrandis l'aile ou vole plus vite.",
  "card.re": "Nombre de Reynolds",
  "card.re.veryLow":
    "Très faible : une aile minuscule ou très lente. L'écoulement reste laminaire et décolle facilement ; les prédictions sont incertaines.",
  "card.re.low":
    "Faible, typique des petits modèles lents : les bulbes de décollement laminaire comptent, et les résultats sont moins fiables sous 100 000.",
  "card.re.model": "La plage des modèles réduits et des drones.",
  "card.re.large": "Grands modèles, planeurs et ULM.",
  "card.re.full": "Avions grandeur nature.",
  "card.loading": "Charge alaire",
  "loading.veryLight": "Très légère, comme un modèle d'intérieur ou de vol libre (moins de 15 g/dm²).",
  "loading.light": "Légère, comme un park flyer ou un planeur (15 à 35 g/dm²).",
  "loading.medium": "Moyenne, comme un modèle de sport ou d'école (35 à 70 g/dm²).",
  "loading.heavy": "Lourde pour un modèle : il vole et se pose vite (70 à 150 g/dm²).",
  "loading.fullSize": "Dans la plage des avions grandeur nature (un Cessna 172 porte environ 690 g/dm²).",
  "card.air": "Air à {altitude}",
  "card.air.text": "{temperature} ; la masse volumique vaut {percent} de sa valeur au niveau de la mer.",
  "card.mach": "Nombre de Mach",
  "card.mach.ok": "Assez faible pour traiter l'air comme incompressible.",
  "card.mach.high": "La compressibilité compte au-dessus de 0,3 : Aero la corrige, jusqu'au Mach critique.",
  "sketch.title": "Ton aile, à l'échelle",
  "sketch.a4": "Feuille A4",
  "sketch.person": "Personne, 1,75 m",
  "level.ok": "OK",
  "level.info": "À noter",
  "level.warning": "Attention",
  "level.danger": "Problème",

  "results.title": "Profils recommandés",
  "results.empty":
    "Pas encore de recommandation. Décris ton vol, puis clique sur « Trouver des profils » : Aero analyse les {count} profils de son catalogue à ton nombre de Reynolds, en quelques secondes.",
  "results.context": "Pour Cl {cl} à Re {re}, classés par : {goal}",
  "results.none":
    "Aucun profil ne fait ce vol avec une marge sûre avant le décrochage. Agrandis l'aile, vole plus vite ou allège l'appareil.",
  "results.lowRe":
    "Sous Re 100 000 les prédictions sont moins fiables : compare plusieurs profils plutôt que de te fier à un seul chiffre.",
  "results.estimate":
    "Aile seule, formule simple de l'aile finie : la traînée du fuselage, de l'empennage et de l'hélice n'est pas comptée.",
  "fit.comfortable": "Marge confortable",
  "fit.tight": "Marge faible",
  "fit.near_stall": "Trop près du décrochage",
  "fit.cannot_fly": "Ne peut pas porter l'appareil",
  "fit.no_data": "Pas atteint à cette portance",
  "cand.stall": "Décroche à",
  "cand.glide": "Finesse",
  "cand.power": "Puissance de l'aile",
  "cand.angle": "Incidence",
  "cand.cd": "Coefficient de traînée",
  "cand.clmax": "Portance maximale",
  "cand.thickness": "Épaisseur",
  "cand.easy": "Facile à construire",
  "cand.hard": "Creux sous l'aile : plus difficile à construire",
  "cand.thin": "Mince : plus difficile à construire",
  "cand.fromLibrary": "De ta bibliothèque",
  "cand.analyse": "L'analyser",
  "cand.optimize": "L'optimiser pour mon vol",
  "cand.failed": "Analyse impossible : {message}",
  "table.title": "Tous les profils",
  "table.airfoil": "Profil",
  "table.fit": "Marge",
  "table.build": "Construction",
  "polar.title": "Polaires de traînée à ton nombre de Reynolds",
  "polar.required": "Cl nécessaire",
  "toast.analyse.title": "Prêt à analyser",
  "toast.analyse": "{name} est dans ta bibliothèque et l'onglet Analyse est réglé sur ton vol (Re {re}, Cl {cl}).",
  "toast.optimize.title": "Prêt à optimiser",
  "toast.optimize":
    "{name} est dans ta bibliothèque. L'optimisation cherche le moins de traînée à Cl {cl} et Re {re}, en gardant au moins son épaisseur.",
};

export const MESSAGES: Record<Lang, Record<MessageKey, string>> = { en, fr };

/** The text of `key` in `lang`, with its `{name}` placeholders replaced. */
export function translate(lang: Lang, key: MessageKey, params: Record<string, string | number> = {}): string {
  return MESSAGES[lang][key].replace(/\{(\w+)\}/g, (whole, name: string) =>
    name in params ? String(params[name]) : whole,
  );
}

/** Placeholders of a text, sorted. */
export function placeholders(text: string): string[] {
  return [...text.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();
}

# Glossaire

Les grandeurs et les termes de l'application Aero, expliqués pour les non-spécialistes. Le même texte est affiché dans l'application (bouton Glossary, et sur les libellés soulignés en pointillés).

_Généré depuis `app/src/lib/glossary.ts` par `npm run glossary` ; ne pas modifier à la main. English version: [glossary.md](glossary.md)._

## Les bases

### Profil

**La section d'une aile, d'une pale d'hélice ou d'une voile, coupée parallèlement à l'écoulement.**

Un profil est la forme 2D que l'on voit en coupant une aile parallèlement à l'écoulement : un bord d'attaque arrondi à l'avant, un bord de fuite aigu (ou presque) à l'arrière, un extrados (dessus) et un intrados (dessous).

Aero étudie cette section 2D, comme si l'aile était infiniment longue : les résultats sont par unité d'envergure. Une vraie aile ajoute des effets 3D (tourbillons marginaux, traînée induite) qui ne sont pas calculés ici.

### Corde (c)

**La longueur du profil, du bord d'attaque au bord de fuite ; les positions sont données en x/c.**

La corde c est la droite qui va du bord d'attaque au bord de fuite. Aero travaille sur des profils ramenés à une corde unité : une position x/c = 0,3 veut dire à 30 % de la corde depuis le bord d'attaque, et une épaisseur t/c = 0,12 veut dire 12 % de la corde.

### Angle d'attaque (incidence) (α)

**L'angle entre la corde et l'écoulement qui arrive, en degrés.**

L'angle d'attaque α (ou incidence) est l'angle entre la corde et la direction de l'air qui arrive. L'augmenter augmente la portance, presque linéairement (environ +0,11 de Cl par degré), jusqu'à ce que l'écoulement se décolle de l'extrados : c'est le décrochage, en général entre 10° et 18°.

Un profil cambré porte déjà à α = 0 ; il atteint la portance nulle à un angle légèrement négatif (environ −2° pour un NACA 2412).

### Polaire

**Les courbes des coefficients d'un profil sur une plage d'angles d'attaque ou de coefficients de portance.**

Une polaire décrit le profil sur toute sa plage de fonctionnement : Cl(α) (courbe de portance), Cl(Cd) (polaire de traînée), Cm(α) et les points de transition. Elle est calculée par un balayage, point après point.

Sur la polaire de traînée Cl(Cd), plus une courbe est à gauche à un Cl donné, moins le profil traîne à ce Cl. Les profils laminaires montrent un « baquet » (drag bucket) : une plage de Cl où la traînée est particulièrement faible.

### Balayage (sweep)

**Une série d'analyses à des angles d'attaque (ou des Cl) régulièrement espacés, qui construit une polaire.**

Un balayage calcule le profil à α = début, début + pas, … jusqu'à la valeur de fin (ou de même en Cl). En visqueux, chaque point part de la couche limite du point précédent, comme les commandes ASEQ et CSEQ de XFOIL : c'est plus rapide et ça converge plus souvent. Les points qui ne convergent pas (typiquement au-delà du décrochage) sont écartés de la polaire.

### XFOIL

**Le programme de référence d'analyse de profils de Mark Drela (MIT), qu'Aero réécrit et avec lequel il est validé.**

XFOIL (1986–2013) est le programme d'analyse de profils le plus utilisé, écrit en Fortran par Mark Drela au MIT. Il couple une méthode de panneaux pour l'écoulement extérieur à un modèle intégral de couche limite, ce qui donne portance, traînée, moment et transition en une fraction de seconde.

Aero est une réécriture de XFOIL 6.996 en Rust ; ses résultats sont comparés à ceux de XFOIL sur des cas de référence, jusqu'aux chiffres que XFOIL affiche.

## Forces et coefficients

### Coefficient sans dimension

**Une force divisée par la pression dynamique et la corde, pour que les résultats ne dépendent ni de la taille ni de la vitesse.**

Les forces augmentent avec la masse volumique de l'air ρ, le carré de la vitesse V et la taille. En divisant une force par unité d'envergure par la pression dynamique q = ½ρV² et par la corde c, on obtient un coefficient qui ne dépend que de la forme, de l'angle d'attaque et des nombres de Reynolds et de Mach. Par exemple la portance par mètre d'envergure vaut L = q · c · Cl.

Exemple : à 20 m/s au niveau de la mer, q ≈ 245 Pa ; une corde de 0,2 m avec Cl = 0,5 porte 245 × 0,2 × 0,5 ≈ 25 N par mètre d'envergure.

### Coefficient de portance (Cl)

**La portance (force perpendiculaire à l'écoulement) sous forme sans dimension ; plus il est grand, plus le profil porte.**

Cl mesure la portance, la force perpendiculaire à l'écoulement qui fait tenir un avion en l'air. Valeurs typiques : 0 à 0,5 en croisière, 1,2 à 1,6 au maximum pour des profils courants (Cl max, juste avant le décrochage), jusqu'à 2 ou plus pour des profils hypersustentateurs à basse vitesse.

Cl croît presque linéairement avec l'angle d'attaque (la pente théorique est 2π par radian, environ 0,11 par degré) jusqu'au décrochage, où il plafonne puis chute.

### Coefficient de traînée (Cd)

**La traînée (force dans le sens de l'écoulement, qui freine) sous forme sans dimension ; plus il est petit, mieux c'est.**

Cd mesure la traînée, la force dans le sens de l'écoulement que le moteur (ou la gravité, pour un planeur) doit vaincre. Pour un profil 2D, elle vient du frottement sur la surface et de la différence de pression créée par l'épaississement de la couche limite et le sillage (voir Cdp et Cdf).

Valeurs typiques : 0,004 à 0,008 pour un bon profil dans sa plage normale, davantage au-delà du décrochage. On parle souvent en « points de traînée » (drag counts) : 1 point = 0,0001, donc Cd = 0,0060 fait 60 points. Cd n'a de sens qu'en mode visqueux : un calcul non visqueux ne donne aucune traînée.

### Traînée de pression et de frottement (Cdp / Cdf)

**Les deux parts de la traînée : Cdf due au frottement, Cdp due au champ de pression (traînée de forme).**

Cdf est le frottement de l'air sur la surface : il est faible tant que la couche limite est laminaire et bien plus fort une fois qu'elle est turbulente, d'où l'intérêt de reculer la transition. Cdp, la traînée de pression (ou de forme), vient du déplacement de l'écoulement par la couche limite et augmente fortement quand l'écoulement décolle, près du décrochage.

Aux angles modérés, le frottement est en général la plus grande part.

### Finesse (L/D)

**Cl divisé par Cd : combien de portance le profil donne par unité de traînée ; plus elle est grande, mieux c'est.**

La finesse L/D = Cl / Cd mesure l'efficacité du profil. Les bons profils atteignent 100 à 150 en 2D (un avion complet, avec son fuselage et les effets 3D, beaucoup moins : environ 15 pour un avion de ligne, 40 à 60 pour un planeur). Son maximum, et le Cl où il se produit, est un chiffre clé pour choisir un profil pour le vol plané ou la distance.

### Coefficient de moment (au quart de corde) (Cm)

**Le moment de tangage autour du point à 25 % de la corde ; négatif signifie que le profil tend à piquer.**

Cm mesure le couple que la pression exerce sur le profil autour d'un point situé au quart de la corde (c/4). Pour les profils cambrés il est négatif et presque constant avant le décrochage : −0,05 pour un NACA 2412, −0,1 ou moins pour des profils très cambrés. Un profil symétrique a Cm ≈ 0.

Il compte pour la stabilité et la structure : un Cm très négatif demande un empennage plus grand (ou du vrillage) pour équilibrer l'avion et tord l'aile ; les ailes volantes demandent des profils à Cm ≥ 0 (bord de fuite relevé, « autostable »).

### Coefficient de pression (Cp)

**La pression sur la surface comparée à la pression de l'écoulement libre ; négatif signifie dépression (aspiration).**

Cp = (p − p∞) / q indique comment la pression locale p se compare à la pression lointaine p∞. Cp = 1 au point d'arrêt (là où l'air s'arrête, au bord d'attaque), Cp < 0 là où l'air va plus vite que l'écoulement libre (dépression, surtout sur l'extrados).

L'aire entre les courbes de l'extrados et de l'intrados est la portance. Par convention le graphique a −Cp vers le haut, pour que l'extrados (dépression) soit en haut. Un pic de dépression aigu suivi d'une forte remontée de pression favorise le décollement ; Cp min est la valeur la plus basse.

### Décrochage

**Au-delà d'un certain angle d'attaque, l'écoulement décolle de l'extrados : la portance chute et la traînée s'envole.**

Quand l'angle d'attaque augmente, la remontée de pression sur l'arrière de l'extrados devient trop forte pour la couche limite, qui décolle. La portance cesse d'augmenter (Cl max) puis chute, la traînée et le moment changent brutalement.

Les écoulements décrochés sont instationnaires et difficiles à calculer : près et au-delà de Cl max, la méthode de XFOIL (et d'Aero) devient approximative et ne converge souvent pas — ces points sont écartés des polaires. Considère les Cl max calculés comme des estimations.

### Moment de charnière (Ch)

**Le moment de la pression sur un volet (la partie en arrière d'une charnière) autour de cette charnière.**

Si l'arrière du profil est une gouverne (volet, aileron, profondeur) articulée autour d'un point, l'air pousse dessus et crée un moment autour de la charnière ; Ch est ce moment divisé par q·c². Il indique l'effort que doit fournir le servo ou le pilote. Ici un Ch positif tend à relever le bord de fuite du volet (convention FMOM de XFOIL).

## Conditions d'écoulement

### Nombre de Reynolds (Re)

**Vitesse × corde / viscosité de l'air : compare l'inertie à la viscosité ; il fixe le comportement de la couche limite.**

Re = V · c / ν, avec ν ≈ 1,5·10⁻⁵ m²/s pour l'air au niveau de la mer. C'est le paramètre le plus important d'une analyse visqueuse : à bas Re la couche limite reste laminaire plus longtemps mais décolle facilement (bulbes laminaires), et la traînée est plus forte ; à haut Re l'écoulement est plus turbulent et le profil atteint un Cl max plus élevé.

Ordres de grandeur : modèles réduits 50 000 – 300 000, planeurs 1 – 3 millions, avions légers 3 – 6 millions, avions de ligne 20 – 50 millions. Exemple : 20 m/s avec une corde de 0,2 m donne Re ≈ 270 000.

### Nombre de Mach (M)

**La vitesse divisée par la vitesse du son ; au-dessus d'environ 0,3, la compressibilité de l'air commence à compter.**

M = V / a, avec a ≈ 340 m/s au niveau de la mer. En dessous de M ≈ 0,3, l'air se comporte comme incompressible et M = 0 convient. Au-dessus, la correction de Kármán–Tsien tient compte de la compressibilité, qui creuse les pics de dépression.

Quand l'écoulement local devient supersonique quelque part sur le profil (Cp min en dessous de la valeur critique Cp*), des ondes de choc apparaissent, que cette méthode ne sait pas représenter : Aero signale alors que le Mach critique est dépassé, sauf si « allow supercritical » est coché (les résultats deviennent alors peu fiables).

### Analyse visqueuse et non visqueuse

**Non visqueux : écoulement idéal sans frottement (portance et moment seulement). Visqueux : ajoute la couche limite, donc la traînée, la transition et le décrochage.**

L'analyse non visqueuse (méthode de panneaux) traite l'air comme un fluide parfait. Elle est instantanée et donne un bon Cl et un bon Cm aux angles modérés, mais aucune traînée et pas de décrochage.

L'analyse visqueuse la couple à un modèle de la fine couche d'air freinée par le frottement près de la surface (la couche limite), comme XFOIL. Elle donne la traînée, les points de transition et une courbe de portance réaliste jusqu'au décrochage. Elle demande un nombre de Reynolds et prend quelques dizaines de millisecondes par point.

## Couche limite et transition

### Couche limite

**La fine couche d'air près de la surface, freinée par le frottement ; elle cause la traînée et le décrochage.**

Au contact de la paroi, l'air colle (vitesse nulle) ; quelques millimètres plus loin, il va à la vitesse extérieure. Cette fine couche est la couche limite. Elle est d'abord laminaire (régulière) près du bord d'attaque, puis devient turbulente (désordonnée, avec plus de frottement) au point de transition. Elle se prolonge derrière le profil par le sillage.

Ses graphiques dans l'onglet Analysis : δ* (épaisseur de déplacement, de combien l'écoulement extérieur est repoussé), θ (épaisseur de quantité de mouvement, liée à la traînée), H = δ*/θ (facteur de forme : environ 2,6 en laminaire, 1,4 en turbulent ; il augmente à l'approche du décollement, au-delà d'environ 3,5 en laminaire ou 2,5 en turbulent), Cf (frottement pariétal), N (amplification des perturbations, voir Ncrit) et Ue (vitesse au bord de la couche).

### Transition (Xtr)

**L'endroit où la couche limite passe de laminaire à turbulente, en fraction de corde (Xtr), sur chaque face.**

De petites perturbations dans la couche limite laminaire grandissent jusqu'à la rendre turbulente. Xtr top et Xtr bot donnent cette position sur l'extrados et l'intrados (1 veut dire laminaire jusqu'au bord de fuite). Comme le frottement turbulent est plusieurs fois plus fort, une transition plus en arrière veut dire moins de traînée : c'est le principe des profils laminaires.

La transition avance sur l'extrados et recule sur l'intrados quand l'angle d'attaque augmente. Elle est prédite par la méthode eᴺ (voir Ncrit) ou forcée par un turbulateur (trip).

### Taux d'amplification critique (Ncrit)

**À quel point l'air qui arrive est perturbé : la transition se produit quand les perturbations ont grandi d'un facteur e^Ncrit. 9 = soufflerie moyenne.**

La méthode eᴺ suit la croissance des perturbations dans la couche laminaire ; N est le logarithme de leur amplification. La transition se produit quand N atteint Ncrit, qui dépend du calme de l'air ambiant : 9 est la valeur habituelle (soufflerie moyenne), 11 à 14 pour un planeur en air calme, 4 à 8 pour un environnement bruyant, une surface rugueuse ou qui vibre.

Un Ncrit plus faible fait transitionner plus tôt, donc plus de traînée. Aero permet de le régler séparément sur chaque face.

### Turbulateur (transition forcée) (Trip x/c)

**Force la transition à un x/c donné, comme une bande de turbulateur ou un bord rugueux. 1 = transition libre.**

Un turbulateur (trip) force la couche limite à devenir turbulente à une position donnée, quoi que prédise la méthode eᴺ — comme un ruban zigzag, une marche ou des salissures (insectes) sur une vraie aile. La transition a alors lieu au turbulateur ou avant. Utile pour représenter des surfaces rugueuses ou comparer à des essais en soufflerie avec turbulateurs ; 1 (par défaut) veut dire transition libre.

### Bulbe de décollement laminaire

**À bas nombre de Reynolds, la couche laminaire décolle, devient turbulente et recolle, laissant un petit bulbe qui ajoute de la traînée.**

En dessous de Re ≈ 500 000, la couche limite laminaire décolle souvent avant d'être devenue turbulente ; la couche décollée devient alors turbulente et recolle en général, en enfermant un bulbe. Il se voit par un plateau dans Cp et une bosse dans δ* et H, et ajoute de la traînée. La méthode de XFOIL est réputée bien prédire ces bulbes — une des raisons de son succès pour les modèles réduits et les petits drones.

## Géométrie

### Désignation NACA

**Des chiffres qui décrivent une famille de profils : NACA 2412 = cambrure 2 % à 40 % de la corde, épaisseur 12 %.**

Les profils NACA à quatre chiffres (années 1930) sont définis par des formules : dans NACA MPTT, M est la cambrure maximale en % de la corde, P sa position en dixièmes de corde et TT l'épaisseur maximale en %. Le NACA 0012 est symétrique et épais de 12 % ; le NACA 4415 a 4 % de cambrure à 40 % et 15 % d'épaisseur.

Les profils à cinq chiffres (ex. 23012) ont leur cambrure maximale plus en avant et un faible moment de tangage : le premier chiffre × 0,15 donne le Cl d'adaptation, le deuxième la position de la cambrure maximale (en vingtièmes de corde), le troisième indique si la ligne moyenne est relevée à l'arrière (1) ou non (0), et les deux derniers l'épaisseur.

### Épaisseur (t/c)

**La plus grande distance entre l'extrados et l'intrados, en fraction de la corde.**

Un profil plus épais est plus solide et plus rigide (le longeron est plus haut, l'aile plus légère) et contient plus de carburant, et il décroche en général plus doucement ; un profil plus mince traîne moins à grande vitesse. Valeurs typiques : 8 – 10 % pour des ailes rapides ou minces, 12 – 15 % pour la plupart des avions, 18 – 21 % à l'emplanture des grandes ailes. Sa position (x/c du maximum) compte pour l'écoulement laminaire.

### Cambrure

**À quel point le profil est courbé : la plus grande distance entre la corde et la ligne moyenne, en fraction de corde.**

La ligne moyenne passe à mi-chemin entre l'extrados et l'intrados ; la cambrure est sa plus grande distance à la corde. La cambrure donne de la portance à incidence nulle et augmente Cl max, au prix d'un Cm plus négatif. Les profils symétriques (0 %) servent pour les empennages et la voltige ; 2 – 4 % est courant pour les ailes, 5 – 8 % pour des profils porteurs à basse vitesse.

### Rayon de bord d'attaque

**Le rayon du cercle qui épouse le nez arrondi, en fraction de la corde.**

Un grand rayon de bord d'attaque adoucit le pic de dépression aux fortes incidences et donne un décrochage plus doux ; un petit rayon (nez pointu) décroche brutalement depuis le bord d'attaque. Pour un NACA 0012, il vaut environ 1,6 % de la corde.

### Angle et épaisseur de bord de fuite

**L'angle entre les deux faces au bord de fuite, et l'écart entre elles s'il n'est pas aigu.**

Un angle de bord de fuite très petit est difficile à fabriquer et fragile ; un bord de fuite épais (un petit écart, souvent 0,1 – 0,5 % de la corde) est plus facile à réaliser et XFOIL le représente par un petit panneau en travers de l'écart.

### Panneaux et maillage

**Le contour du profil est découpé en segments droits (panneaux) sur lesquels on calcule l'écoulement ; 160 est un bon défaut.**

La méthode de panneaux représente la surface par N nœuds reliés par des panneaux droits. La routine PANGEN de XFOIL les place le long d'une spline du contour, plus serrés là où la courbure est forte (bord d'attaque) et au bord de fuite. Plus de panneaux donnent des résultats plus précis mais plus lents ; 160 à 240 nœuds sont habituels. Les résultats ne devraient presque pas changer quand on en augmente le nombre — sinon, raffine.

## Options d'analyse

### α ou Cl imposé

**Soit tu fixes l'angle d'attaque et tu obtiens Cl, soit tu fixes Cl et le solveur trouve l'angle (commande CL de XFOIL).**

Un avion en vol a besoin d'une portance donnée (pour équilibrer son poids), pas d'un angle donné. Imposer Cl permet de comparer des profils équitablement à portance égale : lequel traîne le moins à Cl = 0,5 ? Le solveur trouve alors l'angle d'attaque en même temps que la couche limite.

### Mach et Reynolds variant avec Cl (M, Re)

**Pour un avion donné, voler à un Cl plus grand veut dire voler plus lentement : M et Re diminuent alors (TYPE 2 et 3 de XFOIL).**

Avec « fixed », Re et M sont les mêmes à chaque point. Avec « ∝ 1/√Cl » (TYPE 2 de XFOIL), ils varient comme pour un avion de masse et d'aile fixées : la vitesse, donc M et Re, est proportionnelle à 1/√Cl, et les valeurs saisies sont celles à Cl = 1. Avec « Re ∝ 1/Cl » (TYPE 3), Re varie comme 1/Cl (portance fixée, corde variable). Cela donne des polaires réalistes pour un avion donné.

### Modèle d'amplification (Ampl.)

**Quelle corrélation de croissance des perturbations est utilisée pour la transition : celle d'origine de XFOIL ou la version modifiée de sa commande DAMP.**

Ce sont deux corrélations d'enveloppe de la méthode eᴺ. Celle d'origine est la valeur par défaut de XFOIL et celle de sa validation ; la version modifiée (DAMP) change les taux de croissance des perturbations dans certains états de couche limite. Garde « original » sauf pour comparer à des résultats obtenus avec DAMP.

### Convergence

**La solution visqueuse est trouvée par corrections successives ; elle a convergé quand les corrections deviennent négligeables.**

La couche limite et l'écoulement extérieur sont résolus ensemble par des itérations de Newton (20 au plus pour un point, comme dans XFOIL). Près du décrochage, ou quand la transition hésite entre deux positions, les itérations peuvent ne pas se stabiliser : le point est alors signalé non convergé et ne doit pas être utilisé. L'approcher depuis un point voisin (balayage) aide souvent.

## Optimisation de forme

### Optimisation de forme

**L'ordinateur remodèle le profil en essayant de nombreux candidats, pour améliorer un objectif en respectant des contraintes.**

Tu choisis des points de fonctionnement (Cl ou α, chacun avec un poids), un objectif (le moins de traînée, ou la meilleure finesse) et des contraintes (épaisseur, moment…). L'optimiseur analyse alors des centaines de formes légèrement différentes avec le solveur visqueux et améliore sans cesse la meilleure.

Un profil optimisé pour un seul point est souvent moins bon ailleurs : donne plusieurs points, ou vérifie les polaires ensuite (« Compare polars »).

### Paramétrisation CST (CST)

**Une façon de décrire un profil lisse avec quelques nombres (poids de polynômes), que l'optimiseur fait varier.**

La Class/Shape Transformation (Kulfan) écrit chaque face comme une forme de « classe » à nez rond et queue pointue multipliée par un polynôme lisse. À l'ordre 6, chaque face a 7 poids : 14 nombres décrivent tout le profil. Ils sont d'abord ajustés sur ton profil ; l'erreur d'ajustement est affichée. Un ordre plus élevé permet des formes plus fines mais allonge la recherche.

### Objectif

**Ce que l'optimiseur cherche à améliorer : la traînée moyenne pondérée, ou la finesse moyenne pondérée, sur les points de fonctionnement.**

Avec plusieurs points de fonctionnement, chacun a un poids : un point de poids 2 compte deux fois plus. Un choix typique pour un planeur est un Cl faible (croisière rapide) et un Cl élevé (spirale en thermique), avec des poids qui reflètent le temps passé dans chacun.

### Contraintes et formes admissibles

**Des conditions que le résultat doit respecter (épaisseur minimale…) ; une forme qui en viole une est « non admissible ».**

Sans contraintes, un optimiseur qui réduit la traînée rendrait le profil aussi fin qu'une feuille. Les contraintes le gardent réalisable : épaisseur minimale (globale ou à l'emplacement d'un longeron), Cm minimal, angle de bord de fuite minimal. Une forme qui viole une contrainte, ou dont l'analyse ne converge pas, est non admissible ; une forme admissible bat toujours une forme non admissible, donc le résultat respecte les contraintes dès que c'est possible.

### CMA-ES et Nelder–Mead

**Les deux méthodes de recherche : CMA-ES explore largement avec des populations de candidats, Nelder–Mead affine le meilleur.**

CMA-ES (stratégie d'évolution par adaptation de la matrice de covariance) tire des populations de candidats aléatoires autour du meilleur actuel, et apprend quelles directions sont payantes ; chaque population est analysée en parallèle sur tous les cœurs. Nelder–Mead peaufine ensuite le résultat en déplaçant un petit simplexe de candidats. La graine (seed) fixe les tirages aléatoires : la même graine donne exactement le même résultat.

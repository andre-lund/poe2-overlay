//! Waystone danger texts in the UI languages. The rules match on English mod text and
//! stay keyed by their English label (see `danger::localize`). Russian has been checked
//! in play; the other languages are best-effort translations — corrections welcome.

use crate::trade::Lang;

/// (English label, label, explanation).
type Texts = &'static [(&'static str, &'static str, &'static str)];

/// The translated (label, explanation) for an English danger label, `None` for English
/// or an unknown label.
pub fn text(lang: Lang, en_label: &str) -> Option<(&'static str, &'static str)> {
    let table: Texts = match lang {
        Lang::En => return None,
        Lang::Ru => RU,
        Lang::De => DE,
        Lang::Fr => FR,
        Lang::Es => ES,
        Lang::Pt => PT,
        Lang::Ja => JA,
        Lang::Ko => KO,
        Lang::Th => TH,
    };
    table
        .iter()
        .find(|(en, _, _)| *en == en_label)
        .map(|(_, l, w)| (*l, *w))
}

const RU: Texts = &[

    ("Extra Chaos damage", "Дополнительный урон хаосом",
        "Монстры добавляют хаос к каждому удару — он проходит сквозь энергетический щит, и от него трудно защититься"),
    ("Extra Fire damage", "Дополнительный урон от огня", "Монстры добавляют урон от огня к каждому удару"),
    ("Extra Cold damage", "Дополнительный урон от холода", "Монстры добавляют урон от холода к каждому удару"),
    ("Extra Lightning damage", "Дополнительный урон от молнии", "Монстры добавляют урон от молнии к каждому удару"),
    ("Monsters deal more damage", "Монстры наносят больше урона", "Получаемый урон выше во всём"),
    ("Extra projectiles", "Дополнительные снаряды", "Попадания нескольких снарядов сразу могут убить мгновенно"),
    ("Monster crit chance", "Шанс крита монстров", "Монстры критуют гораздо чаще — внезапные всплески урона"),
    ("Monster crit damage", "Урон крита монстров", "Критические удары монстров бьют намного сильнее"),
    ("Monster speed", "Скорость монстров", "Быстрее атаки, чары и передвижение — труднее уклоняться"),
    ("Reduced recovery", "Сниженное восстановление",
        "Восстановление здоровья и щита урезано — не вытянуть против всплесков урона"),
    ("Elemental Weakness curse", "Проклятие «Уязвимость к стихиям»", "Снижает ваши сопротивления стихиям"),
    ("Mark for Death", "Метка смерти",
        "Получаемый урон повышен после убийства редкого/уникального монстра — можно умереть цепочкой"),
    ("Delirium", "Делириум", "Более крепкие и смертоносные монстры Делириума"),
    ("Periodic no-damage", "Периоды без урона",
        "3 секунды из каждых 10 вы не наносите урона — получаете удары, не имея возможности ответить"),
    ("Temporal Chains curse", "Проклятие «Оковы времени»", "Замедляет ваши действия, восстановление и действие флаконов"),
    ("Enfeeble curse", "Проклятие «Ослабление»", "Снижает ваш урон и меткость"),
    ("Reduced flask charges", "Меньше зарядов флаконов", "Флаконы реже доступны по ходу карты"),
    ("Poison on hit", "Отравление при попадании", "Накапливающийся урон хаосом со временем"),
    ("Bleed on hit", "Кровотечение при попадании", "Физический урон со временем, сильнее при движении"),
    ("Charge steal", "Кража зарядов", "Отнимает заряды силы, ярости и выносливости посреди боя"),
    ("Armour break", "Раскол брони", "Билды на броне теряют защиту по ходу боя"),
    ("Grasping Vines", "Цепкие лозы", "Обездвиживают и замедляют вблизи монстров"),
    ("Flammability", "Воспламеняемость", "Повышает получаемый урон от огня"),
    ("Shock", "Шок", "Шок повышает получаемый урон"),
    ("Freeze", "Заморозка", "Заморозка может обездвижить"),
    ("Stun", "Оглушение", "Выше риск попасть в цепочку оглушений"),
    ("Monster accuracy", "Меткость монстров", "Монстры реже промахиваются"),
    ("Monster AoE", "Область действия монстров", "Шире зоны поражения — труднее уклониться"),
    ("Combo: crit chance + crit damage", "Комбо: шанс и урон крита",
        "Монстры часто критуют И бьют сильно — высокий риск умереть с одного удара"),
    ("Combo: no sustain + amplified damage", "Комбо: нет восстановления + усиленный урон",
        "Восстановление урезано, а получаемый урон усилен"),
];

const DE: Texts = &[
    ("Extra Chaos damage", "Zusätzlicher Chaosschaden", "Monster fügen jedem Treffer Chaos hinzu — umgeht Energieschild und ist schwer zu widerstehen"),
    ("Extra Fire damage", "Zusätzlicher Feuerschaden", "Monster fügen jedem Treffer Feuerschaden hinzu"),
    ("Extra Cold damage", "Zusätzlicher Kälteschaden", "Monster fügen jedem Treffer Kälteschaden hinzu"),
    ("Extra Lightning damage", "Zusätzlicher Blitzschaden", "Monster fügen jedem Treffer Blitzschaden hinzu"),
    ("Monsters deal more damage", "Monster verursachen mehr Schaden", "Höherer erlittener Schaden auf ganzer Linie"),
    ("Extra projectiles", "Zusätzliche Projektile", "Überlappende Projektiltreffer können dich sofort töten"),
    ("Monster crit chance", "Kritische Trefferchance der Monster", "Monster treffen viel öfter kritisch — plötzliche Schadensspitzen"),
    ("Monster crit damage", "Kritischer Schaden der Monster", "Kritische Treffer der Monster schlagen viel härter zu"),
    ("Monster speed", "Monstergeschwindigkeit", "Schnellere Angriffe, Zauber und Bewegung — schwerer auszuweichen"),
    ("Reduced recovery", "Verringerte Erholung", "Deine Leben-/ES-Erholung ist gekürzt — keine Erholung gegen Schadensspitzen"),
    ("Elemental Weakness curse", "Fluch: Elementarschwäche", "Senkt deine Elementarwiderstände"),
    ("Mark for Death", "Todesmal", "Erhöhter erlittener Schaden nach dem Töten eines seltenen/einzigartigen Monsters — Kettentod möglich"),
    ("Delirium", "Delirium", "Zähere, tödlichere Delirium-Monster im Gebiet"),
    ("Periodic no-damage", "Phasen ohne Schaden", "3 von 10 Sekunden verursachst du keinen Schaden — du wirst getroffen, ohne zurückschlagen zu können"),
    ("Temporal Chains curse", "Fluch: Zeitliche Ketten", "Verlangsamt deine Aktionen, Erholung und Fläschchenwirkung"),
    ("Enfeeble curse", "Fluch: Entkräftung", "Verringert deinen Schaden und deine Genauigkeit"),
    ("Reduced flask charges", "Weniger Fläschchenladungen", "Weniger Fläschchen-Erholung im Verlauf der Karte"),
    ("Poison on hit", "Vergiftung bei Treffer", "Stapelnder Chaosschaden über Zeit"),
    ("Bleed on hit", "Blutung bei Treffer", "Physischer Schaden über Zeit, schlimmer in Bewegung"),
    ("Charge steal", "Ladungsraub", "Raubt mitten im Kampf deine Energie-, Raserei- und Widerstandsladungen"),
    ("Armour break", "Rüstungsbruch", "Rüstungs-Builds verlieren im Kampfverlauf Schadensminderung"),
    ("Grasping Vines", "Greifende Ranken", "Fesseln und verlangsamen dich im Nahkampf"),
    ("Flammability", "Entflammbarkeit", "Erhöht den erlittenen Feuerschaden"),
    ("Shock", "Schock", "Schock erhöht den erlittenen Schaden"),
    ("Freeze", "Einfrieren", "Einfrieren kann dich festsetzen"),
    ("Stun", "Betäubung", "Höheres Risiko, dauerhaft betäubt zu werden"),
    ("Monster accuracy", "Genauigkeit der Monster", "Monster verfehlen dich seltener"),
    ("Monster AoE", "Flächenwirkung der Monster", "Größere Trefferflächen — schwerer auszuweichen"),
    ("Combo: crit chance + crit damage", "Kombo: Kritchance + Kritschaden", "Monster treffen oft kritisch UND hart — hohes One-Shot-Risiko"),
    ("Combo: no sustain + amplified damage", "Kombo: keine Erholung + verstärkter Schaden", "Deine Erholung ist gekürzt, während der erlittene Schaden verstärkt ist"),
];

const FR: Texts = &[
    ("Extra Chaos damage", "Dégâts de chaos supplémentaires", "Les monstres ajoutent du chaos à chaque coup — il ignore le bouclier d'énergie et se résiste mal"),
    ("Extra Fire damage", "Dégâts de feu supplémentaires", "Les monstres ajoutent des dégâts de feu à chaque coup"),
    ("Extra Cold damage", "Dégâts de froid supplémentaires", "Les monstres ajoutent des dégâts de froid à chaque coup"),
    ("Extra Lightning damage", "Dégâts de foudre supplémentaires", "Les monstres ajoutent des dégâts de foudre à chaque coup"),
    ("Monsters deal more damage", "Monstres plus puissants", "Dégâts subis plus élevés dans l'ensemble"),
    ("Extra projectiles", "Projectiles supplémentaires", "Les impacts de projectiles cumulés peuvent vous tuer net"),
    ("Monster crit chance", "Chance de critique des monstres", "Les monstres font bien plus de critiques — pics de dégâts soudains"),
    ("Monster crit damage", "Dégâts critiques des monstres", "Les critiques des monstres frappent beaucoup plus fort"),
    ("Monster speed", "Vitesse des monstres", "Attaques, sorts et déplacements plus rapides — plus durs à esquiver"),
    ("Reduced recovery", "Récupération réduite", "Votre récupération de vie/BE est réduite — impossible de tenir face aux pics"),
    ("Elemental Weakness curse", "Malédiction Faiblesse élémentaire", "Réduit vos résistances élémentaires"),
    ("Mark for Death", "Marque de la mort", "Dégâts subis accrus après avoir tué un monstre rare/unique — morts en chaîne possibles"),
    ("Delirium", "Délire", "Monstres du Délire plus résistants et plus mortels"),
    ("Periodic no-damage", "Phases sans dégâts", "3 secondes sur 10 vous n'infligez aucun dégât — vous encaissez sans pouvoir riposter"),
    ("Temporal Chains curse", "Malédiction Chaînes temporelles", "Ralentit vos actions, votre récupération et l'effet des flacons"),
    ("Enfeeble curse", "Malédiction Affaiblissement", "Réduit vos dégâts et votre précision"),
    ("Reduced flask charges", "Moins de charges de flacon", "Moins de récupération par flacons sur la carte"),
    ("Poison on hit", "Poison à l'impact", "Dégâts de chaos dans le temps qui se cumulent"),
    ("Bleed on hit", "Saignement à l'impact", "Dégâts physiques dans le temps, pires en mouvement"),
    ("Charge steal", "Vol de charges", "Vole vos charges de puissance, de frénésie et d'endurance en plein combat"),
    ("Armour break", "Brise-armure", "Les builds armure perdent leur atténuation au fil du combat"),
    ("Grasping Vines", "Lianes agrippantes", "Vous immobilisent et vous ralentissent au corps à corps"),
    ("Flammability", "Inflammabilité", "Augmente les dégâts de feu subis"),
    ("Shock", "Choc", "Le choc augmente les dégâts subis"),
    ("Freeze", "Gel", "Le gel peut vous immobiliser"),
    ("Stun", "Étourdissement", "Risque accru d'étourdissements en chaîne"),
    ("Monster accuracy", "Précision des monstres", "Les monstres vous ratent moins souvent"),
    ("Monster AoE", "Zone d'effet des monstres", "Zones d'impact plus grandes — plus dures à esquiver"),
    ("Combo: crit chance + crit damage", "Combo : chance + dégâts critiques", "Les monstres critiquent souvent ET frappent fort — gros risque de mort en un coup"),
    ("Combo: no sustain + amplified damage", "Combo : pas de récupération + dégâts amplifiés", "Votre récupération est réduite pendant que les dégâts subis sont amplifiés"),
];

const ES: Texts = &[
    ("Extra Chaos damage", "Daño de caos adicional", "Los monstruos añaden caos a cada golpe — ignora el escudo de energía y es difícil de resistir"),
    ("Extra Fire damage", "Daño de fuego adicional", "Los monstruos añaden daño de fuego a cada golpe"),
    ("Extra Cold damage", "Daño de frío adicional", "Los monstruos añaden daño de frío a cada golpe"),
    ("Extra Lightning damage", "Daño de rayo adicional", "Los monstruos añaden daño de rayo a cada golpe"),
    ("Monsters deal more damage", "Los monstruos infligen más daño", "Más daño recibido en general"),
    ("Extra projectiles", "Proyectiles adicionales", "Varios impactos de proyectiles a la vez pueden matarte de golpe"),
    ("Monster crit chance", "Probabilidad de crítico de monstruos", "Los monstruos aciertan críticos mucho más a menudo — picos de daño repentinos"),
    ("Monster crit damage", "Daño crítico de monstruos", "Los críticos de los monstruos golpean mucho más fuerte"),
    ("Monster speed", "Velocidad de monstruos", "Ataques, hechizos y movimiento más rápidos — más difíciles de esquivar"),
    ("Reduced recovery", "Recuperación reducida", "Tu recuperación de vida/escudo se reduce — no aguantas los picos de daño"),
    ("Elemental Weakness curse", "Maldición Debilidad elemental", "Reduce tus resistencias elementales"),
    ("Mark for Death", "Marca de la muerte", "Más daño recibido tras matar un monstruo raro/único — muertes en cadena"),
    ("Delirium", "Delirio", "Monstruos del Delirio más duros y letales"),
    ("Periodic no-damage", "Periodos sin daño", "3 de cada 10 segundos no infliges daño — recibes golpes sin poder responder"),
    ("Temporal Chains curse", "Maldición Cadenas temporales", "Ralentiza tus acciones, recuperación y efecto de frascos"),
    ("Enfeeble curse", "Maldición Debilitar", "Reduce tu daño y precisión"),
    ("Reduced flask charges", "Menos cargas de frasco", "Menos recuperación de frascos durante el mapa"),
    ("Poison on hit", "Envenenamiento al golpear", "Daño de caos acumulable con el tiempo"),
    ("Bleed on hit", "Sangrado al golpear", "Daño físico con el tiempo, peor en movimiento"),
    ("Charge steal", "Robo de cargas", "Te roba cargas de poder, frenesí y aguante en pleno combate"),
    ("Armour break", "Rotura de armadura", "Las builds de armadura pierden mitigación durante el combate"),
    ("Grasping Vines", "Enredaderas aferradoras", "Te inmovilizan y ralentizan cuerpo a cuerpo"),
    ("Flammability", "Inflamabilidad", "Aumenta el daño de fuego recibido"),
    ("Shock", "Electrocución", "La electrocución aumenta el daño recibido"),
    ("Freeze", "Congelación", "La congelación puede inmovilizarte"),
    ("Stun", "Aturdimiento", "Más riesgo de aturdimientos encadenados"),
    ("Monster accuracy", "Precisión de monstruos", "Los monstruos fallan menos"),
    ("Monster AoE", "Área de efecto de monstruos", "Áreas de impacto más grandes — más difíciles de esquivar"),
    ("Combo: crit chance + crit damage", "Combo: probabilidad + daño crítico", "Los monstruos hacen críticos a menudo Y golpean fuerte — gran riesgo de morir de un golpe"),
    ("Combo: no sustain + amplified damage", "Combo: sin recuperación + daño amplificado", "Tu recuperación se reduce mientras el daño recibido aumenta"),
];

const PT: Texts = &[
    (
        "Extra Chaos damage",
        "Dano de caos adicional",
        "Monstros adicionam caos a cada golpe — ignora o escudo de energia e é difícil de resistir",
    ),
    (
        "Extra Fire damage",
        "Dano de fogo adicional",
        "Monstros adicionam dano de fogo a cada golpe",
    ),
    (
        "Extra Cold damage",
        "Dano de gelo adicional",
        "Monstros adicionam dano de gelo a cada golpe",
    ),
    (
        "Extra Lightning damage",
        "Dano de raio adicional",
        "Monstros adicionam dano de raio a cada golpe",
    ),
    (
        "Monsters deal more damage",
        "Monstros causam mais dano",
        "Mais dano recebido em geral",
    ),
    (
        "Extra projectiles",
        "Projéteis adicionais",
        "Vários projéteis ao mesmo tempo podem matar de uma vez",
    ),
    (
        "Monster crit chance",
        "Chance de crítico dos monstros",
        "Monstros acertam críticos com muito mais frequência — picos de dano repentinos",
    ),
    (
        "Monster crit damage",
        "Dano crítico dos monstros",
        "Críticos dos monstros batem muito mais forte",
    ),
    (
        "Monster speed",
        "Velocidade dos monstros",
        "Ataques, feitiços e movimento mais rápidos — mais difíceis de evitar",
    ),
    (
        "Reduced recovery",
        "Recuperação reduzida",
        "Sua recuperação de vida/escudo é cortada — não segura picos de dano",
    ),
    (
        "Elemental Weakness curse",
        "Maldição Fraqueza Elemental",
        "Reduz suas resistências elementais",
    ),
    (
        "Mark for Death",
        "Marca da Morte",
        "Mais dano recebido após matar um monstro raro/único — mortes em cadeia",
    ),
    (
        "Delirium",
        "Delírio",
        "Monstros do Delírio mais resistentes e letais",
    ),
    (
        "Periodic no-damage",
        "Períodos sem dano",
        "3 de cada 10 segundos você não causa dano — apanha sem poder revidar",
    ),
    (
        "Temporal Chains curse",
        "Maldição Correntes Temporais",
        "Desacelera suas ações, recuperação e efeito de frascos",
    ),
    (
        "Enfeeble curse",
        "Maldição Enfraquecer",
        "Reduz seu dano e precisão",
    ),
    (
        "Reduced flask charges",
        "Menos cargas de frasco",
        "Menos recuperação de frascos ao longo do mapa",
    ),
    (
        "Poison on hit",
        "Veneno ao acertar",
        "Dano de caos acumulativo ao longo do tempo",
    ),
    (
        "Bleed on hit",
        "Sangramento ao acertar",
        "Dano físico ao longo do tempo, pior em movimento",
    ),
    (
        "Charge steal",
        "Roubo de cargas",
        "Rouba suas cargas de poder, frenesi e vigor no meio da luta",
    ),
    (
        "Armour break",
        "Quebra de armadura",
        "Builds de armadura perdem mitigação durante a luta",
    ),
    (
        "Grasping Vines",
        "Vinhas Agarradoras",
        "Prendem e desaceleram você no corpo a corpo",
    ),
    (
        "Flammability",
        "Inflamabilidade",
        "Aumenta o dano de fogo recebido",
    ),
    ("Shock", "Choque", "Choque aumenta o dano recebido"),
    (
        "Freeze",
        "Congelamento",
        "Congelamento pode prender você no lugar",
    ),
    (
        "Stun",
        "Atordoamento",
        "Mais risco de atordoamentos em sequência",
    ),
    (
        "Monster accuracy",
        "Precisão dos monstros",
        "Monstros erram menos",
    ),
    (
        "Monster AoE",
        "Área de efeito dos monstros",
        "Áreas de impacto maiores — mais difíceis de evitar",
    ),
    (
        "Combo: crit chance + crit damage",
        "Combo: chance + dano crítico",
        "Monstros acertam críticos com frequência E batem forte — alto risco de morte instantânea",
    ),
    (
        "Combo: no sustain + amplified damage",
        "Combo: sem recuperação + dano ampliado",
        "Sua recuperação é cortada enquanto o dano recebido é ampliado",
    ),
];

const JA: Texts = &[
    (
        "Extra Chaos damage",
        "追加カオスダメージ",
        "モンスターの全ての攻撃にカオスが加わる — エナジーシールドを貫通し、耐性も上げにくい",
    ),
    (
        "Extra Fire damage",
        "追加火ダメージ",
        "モンスターの全ての攻撃に火ダメージが加わる",
    ),
    (
        "Extra Cold damage",
        "追加冷気ダメージ",
        "モンスターの全ての攻撃に冷気ダメージが加わる",
    ),
    (
        "Extra Lightning damage",
        "追加雷ダメージ",
        "モンスターの全ての攻撃に雷ダメージが加わる",
    ),
    (
        "Monsters deal more damage",
        "モンスターの与ダメージ増加",
        "受けるダメージが全体的に増える",
    ),
    (
        "Extra projectiles",
        "追加投射物",
        "投射物が重なって当たると一瞬で倒される",
    ),
    (
        "Monster crit chance",
        "モンスターのクリティカル率",
        "モンスターのクリティカルが大幅に増える — 突然の大ダメージ",
    ),
    (
        "Monster crit damage",
        "モンスターのクリティカルダメージ",
        "モンスターのクリティカルが遥かに重くなる",
    ),
    (
        "Monster speed",
        "モンスターの速度",
        "攻撃・詠唱・移動が速くなり、避けにくい",
    ),
    (
        "Reduced recovery",
        "回復減少",
        "ライフ/ESの回復が減る — 瞬間的なダメージに耐えられない",
    ),
    (
        "Elemental Weakness curse",
        "呪い: エレメンタルウィークネス",
        "元素耐性が下がる",
    ),
    (
        "Mark for Death",
        "死の印",
        "レア/ユニークを倒した後に被ダメージ増加 — 連鎖して倒れる危険",
    ),
    (
        "Delirium",
        "デリリウム",
        "より硬く危険なデリリウムモンスター",
    ),
    (
        "Periodic no-damage",
        "ダメージを与えられない時間",
        "10秒中3秒ダメージを与えられない — 反撃できないまま攻撃を受ける",
    ),
    (
        "Temporal Chains curse",
        "呪い: テンポラルチェーン",
        "行動・回復・フラスコ効果が遅くなる",
    ),
    (
        "Enfeeble curse",
        "呪い: エンフィーブル",
        "与ダメージと命中力が下がる",
    ),
    (
        "Reduced flask charges",
        "フラスコチャージ減少",
        "マップ中のフラスコ回復が減る",
    ),
    (
        "Poison on hit",
        "ヒット時に毒",
        "蓄積するカオスの継続ダメージ",
    ),
    (
        "Bleed on hit",
        "ヒット時に出血",
        "物理の継続ダメージ、移動中は悪化",
    ),
    (
        "Charge steal",
        "チャージ奪取",
        "戦闘中にパワー/フレンジー/エンデュランスチャージを奪われる",
    ),
    (
        "Armour break",
        "アーマー破壊",
        "戦闘が続くとアーマービルドの軽減が失われる",
    ),
    ("Grasping Vines", "絡みつく蔓", "近接距離で拘束・減速される"),
    ("Flammability", "フラマビリティ", "受ける火ダメージが増える"),
    ("Shock", "感電", "感電で受けるダメージが増える"),
    ("Freeze", "凍結", "凍結で動けなくなる"),
    ("Stun", "スタン", "スタンが連続する危険が高まる"),
    (
        "Monster accuracy",
        "モンスターの命中力",
        "モンスターの攻撃が外れにくくなる",
    ),
    (
        "Monster AoE",
        "モンスターの範囲",
        "攻撃範囲が広がり、避けにくい",
    ),
    (
        "Combo: crit chance + crit damage",
        "コンボ: クリ率 + クリダメ",
        "モンスターが頻繁にクリティカルを出し、しかも重い — 一撃死の危険大",
    ),
    (
        "Combo: no sustain + amplified damage",
        "コンボ: 回復なし + 被ダメ増加",
        "回復が減る一方で受けるダメージが増える",
    ),
];

const KO: Texts = &[
    (
        "Extra Chaos damage",
        "추가 카오스 피해",
        "몬스터의 모든 공격에 카오스가 추가됨 — 에너지 보호막을 무시하고 저항하기 어려움",
    ),
    (
        "Extra Fire damage",
        "추가 화염 피해",
        "몬스터의 모든 공격에 화염 피해가 추가됨",
    ),
    (
        "Extra Cold damage",
        "추가 냉기 피해",
        "몬스터의 모든 공격에 냉기 피해가 추가됨",
    ),
    (
        "Extra Lightning damage",
        "추가 번개 피해",
        "몬스터의 모든 공격에 번개 피해가 추가됨",
    ),
    (
        "Monsters deal more damage",
        "몬스터 피해 증가",
        "전반적으로 받는 피해가 늘어남",
    ),
    (
        "Extra projectiles",
        "추가 투사체",
        "투사체가 겹쳐 맞으면 순식간에 죽을 수 있음",
    ),
    (
        "Monster crit chance",
        "몬스터 치명타 확률",
        "몬스터가 훨씬 자주 치명타를 냄 — 갑작스러운 큰 피해",
    ),
    (
        "Monster crit damage",
        "몬스터 치명타 피해",
        "몬스터 치명타가 훨씬 강해짐",
    ),
    (
        "Monster speed",
        "몬스터 속도",
        "공격·시전·이동이 빨라져 피하기 어려움",
    ),
    (
        "Reduced recovery",
        "회복 감소",
        "생명력/보호막 회복이 줄어 순간 피해를 버티기 어려움",
    ),
    (
        "Elemental Weakness curse",
        "저주: 원소 약화",
        "원소 저항이 낮아짐",
    ),
    (
        "Mark for Death",
        "죽음의 표식",
        "희귀/고유 몬스터 처치 후 받는 피해 증가 — 연쇄 사망 위험",
    ),
    ("Delirium", "환영", "더 강하고 치명적인 환영 몬스터"),
    (
        "Periodic no-damage",
        "피해를 줄 수 없는 시간",
        "10초 중 3초 동안 피해를 줄 수 없음 — 반격하지 못하고 맞게 됨",
    ),
    (
        "Temporal Chains curse",
        "저주: 시간의 사슬",
        "행동, 회복, 플라스크 효과가 느려짐",
    ),
    ("Enfeeble curse", "저주: 쇠약", "피해와 정확도가 낮아짐"),
    (
        "Reduced flask charges",
        "플라스크 충전 감소",
        "맵 진행 중 플라스크 회복이 줄어듦",
    ),
    ("Poison on hit", "적중 시 중독", "중첩되는 카오스 지속 피해"),
    (
        "Bleed on hit",
        "적중 시 출혈",
        "물리 지속 피해, 이동 중 악화",
    ),
    (
        "Charge steal",
        "충전 강탈",
        "전투 중 권능/격분/인내 충전을 빼앗김",
    ),
    (
        "Armour break",
        "방어도 파괴",
        "전투가 길어질수록 방어도 빌드의 경감이 사라짐",
    ),
    (
        "Grasping Vines",
        "움켜쥐는 덩굴",
        "근접 거리에서 속박되고 느려짐",
    ),
    ("Flammability", "인화성", "받는 화염 피해가 증가함"),
    ("Shock", "감전", "감전은 받는 피해를 증가시킴"),
    ("Freeze", "동결", "동결로 움직이지 못할 수 있음"),
    ("Stun", "기절", "연속 기절 위험 증가"),
    ("Monster accuracy", "몬스터 정확도", "몬스터가 덜 빗나감"),
    (
        "Monster AoE",
        "몬스터 범위",
        "피격 범위가 넓어져 피하기 어려움",
    ),
    (
        "Combo: crit chance + crit damage",
        "콤보: 치명타 확률 + 치명타 피해",
        "몬스터가 자주 치명타를 내고 강하게 때림 — 한 방에 죽을 위험 큼",
    ),
    (
        "Combo: no sustain + amplified damage",
        "콤보: 회복 없음 + 피해 증폭",
        "회복이 줄어든 상태에서 받는 피해가 증폭됨",
    ),
];

const TH: Texts = &[
    (
        "Extra Chaos damage",
        "ความเสียหายเคออสเพิ่มเติม",
        "มอนสเตอร์เพิ่มเคออสในทุกการโจมตี — ทะลุเกราะพลังงานและต้านทานยาก",
    ),
    (
        "Extra Fire damage",
        "ความเสียหายไฟเพิ่มเติม",
        "มอนสเตอร์เพิ่มความเสียหายไฟในทุกการโจมตี",
    ),
    (
        "Extra Cold damage",
        "ความเสียหายน้ำแข็งเพิ่มเติม",
        "มอนสเตอร์เพิ่มความเสียหายน้ำแข็งในทุกการโจมตี",
    ),
    (
        "Extra Lightning damage",
        "ความเสียหายสายฟ้าเพิ่มเติม",
        "มอนสเตอร์เพิ่มความเสียหายสายฟ้าในทุกการโจมตี",
    ),
    (
        "Monsters deal more damage",
        "มอนสเตอร์สร้างความเสียหายมากขึ้น",
        "ได้รับความเสียหายสูงขึ้นโดยรวม",
    ),
    (
        "Extra projectiles",
        "โปรเจกไทล์เพิ่มเติม",
        "โปรเจกไทล์หลายลูกพร้อมกันอาจฆ่าได้ในทันที",
    ),
    (
        "Monster crit chance",
        "โอกาสคริติคอลของมอนสเตอร์",
        "มอนสเตอร์ติดคริติคอลบ่อยขึ้นมาก — ความเสียหายพุ่งกะทันหัน",
    ),
    (
        "Monster crit damage",
        "ความเสียหายคริติคอลของมอนสเตอร์",
        "คริติคอลของมอนสเตอร์แรงขึ้นมาก",
    ),
    (
        "Monster speed",
        "ความเร็วมอนสเตอร์",
        "โจมตี ร่าย และเคลื่อนที่เร็วขึ้น — หลบยากขึ้น",
    ),
    (
        "Reduced recovery",
        "การฟื้นฟูลดลง",
        "การฟื้นฟูไลฟ์/เกราะพลังงานลดลง — รับความเสียหายพุ่งไม่ไหว",
    ),
    (
        "Elemental Weakness curse",
        "คำสาป Elemental Weakness",
        "ลดค่าต้านทานธาตุของคุณ",
    ),
    (
        "Mark for Death",
        "ตราแห่งความตาย",
        "รับความเสียหายเพิ่มหลังฆ่ามอนสเตอร์แรร์/ยูนีค — อาจตายต่อเนื่อง",
    ),
    (
        "Delirium",
        "Delirium",
        "มอนสเตอร์ Delirium ที่ทนทานและอันตรายกว่า",
    ),
    (
        "Periodic no-damage",
        "ช่วงที่ทำความเสียหายไม่ได้",
        "3 ใน 10 วินาทีคุณทำความเสียหายไม่ได้ — โดนโจมตีโดยไม่สามารถสู้กลับ",
    ),
    (
        "Temporal Chains curse",
        "คำสาป Temporal Chains",
        "ทำให้การกระทำ การฟื้นฟู และผลของขวดยาช้าลง",
    ),
    (
        "Enfeeble curse",
        "คำสาป Enfeeble",
        "ลดความเสียหายและความแม่นยำของคุณ",
    ),
    (
        "Reduced flask charges",
        "ประจุขวดยาลดลง",
        "ฟื้นฟูจากขวดยาได้น้อยลงตลอดแผนที่",
    ),
    (
        "Poison on hit",
        "พิษเมื่อโจมตีโดน",
        "ความเสียหายเคออสต่อเนื่องที่สะสมได้",
    ),
    (
        "Bleed on hit",
        "เลือดไหลเมื่อโจมตีโดน",
        "ความเสียหายกายภาพต่อเนื่อง แย่ลงเมื่อเคลื่อนที่",
    ),
    (
        "Charge steal",
        "ขโมยประจุ",
        "ขโมยประจุพลัง ความคลั่ง และความอดทนของคุณระหว่างต่อสู้",
    ),
    (
        "Armour break",
        "เกราะแตก",
        "บิลด์เกราะสูญเสียการลดความเสียหายระหว่างต่อสู้",
    ),
    (
        "Grasping Vines",
        "เถาวัลย์รัดตรึง",
        "ตรึงและทำให้คุณช้าลงในระยะประชิด",
    ),
    ("Flammability", "Flammability", "เพิ่มความเสียหายไฟที่คุณได้รับ"),
    ("Shock", "ช็อก", "ช็อกเพิ่มความเสียหายที่ได้รับ"),
    ("Freeze", "แช่แข็ง", "การแช่แข็งอาจทำให้ขยับไม่ได้"),
    ("Stun", "มึนงง", "เสี่ยงติดมึนงงต่อเนื่องมากขึ้น"),
    (
        "Monster accuracy",
        "ความแม่นยำของมอนสเตอร์",
        "มอนสเตอร์โจมตีพลาดน้อยลง",
    ),
    (
        "Monster AoE",
        "พื้นที่โจมตีของมอนสเตอร์",
        "พื้นที่โจมตีกว้างขึ้น — หลบยากขึ้น",
    ),
    (
        "Combo: crit chance + crit damage",
        "คอมโบ: โอกาส + ความเสียหายคริติคอล",
        "มอนสเตอร์คริติคอลบ่อยและแรง — เสี่ยงตายในทีเดียวสูง",
    ),
    (
        "Combo: no sustain + amplified damage",
        "คอมโบ: ไม่มีการฟื้นฟู + ความเสียหายเพิ่ม",
        "การฟื้นฟูของคุณถูกลดลงขณะที่ความเสียหายที่ได้รับถูกขยาย",
    ),
];

//! The words a duration is written with in English, French, Spanish, Italian
//! and Portuguese, curated from rustling-ontology's duration and number
//! grammars (MIT or Apache-2.0).

/// Number words from zero to nineteen, and the Spanish twenties written as
/// one word.
pub const SINGLES: &[(&str, u64)] = &[
    ("zero", 0),
    ("one", 1),
    ("two", 2),
    ("three", 3),
    ("four", 4),
    ("five", 5),
    ("six", 6),
    ("seven", 7),
    ("eight", 8),
    ("nine", 9),
    ("ten", 10),
    ("eleven", 11),
    ("twelve", 12),
    ("thirteen", 13),
    ("fourteen", 14),
    ("fifteen", 15),
    ("sixteen", 16),
    ("seventeen", 17),
    ("eighteen", 18),
    ("nineteen", 19),
    ("zéro", 0),
    ("un", 1),
    ("une", 1),
    ("deux", 2),
    ("trois", 3),
    ("quatre", 4),
    ("cinq", 5),
    ("sept", 7),
    ("huit", 8),
    ("neuf", 9),
    ("dix", 10),
    ("onze", 11),
    ("douze", 12),
    ("treize", 13),
    ("quatorze", 14),
    ("quinze", 15),
    ("seize", 16),
    ("cero", 0),
    ("uno", 1),
    ("una", 1),
    ("dos", 2),
    ("tres", 3),
    ("cuatro", 4),
    ("cinco", 5),
    ("seis", 6),
    ("siete", 7),
    ("ocho", 8),
    ("nueve", 9),
    ("diez", 10),
    ("once", 11),
    ("doce", 12),
    ("trece", 13),
    ("catorce", 14),
    ("dieciséis", 16),
    ("dieciseis", 16),
    ("diecisiete", 17),
    ("dieciocho", 18),
    ("diecinueve", 19),
    ("veintiuno", 21),
    ("veintiún", 21),
    ("veintiun", 21),
    ("veintiuna", 21),
    ("veintidós", 22),
    ("veintidos", 22),
    ("veintitrés", 23),
    ("veintitres", 23),
    ("veinticuatro", 24),
    ("veinticinco", 25),
    ("veintiséis", 26),
    ("veintiseis", 26),
    ("veintisiete", 27),
    ("veintiocho", 28),
    ("veintinueve", 29),
    ("due", 2),
    ("tre", 3),
    ("tré", 3),
    ("quattro", 4),
    ("cinque", 5),
    ("sei", 6),
    ("sette", 7),
    ("otto", 8),
    ("nove", 9),
    ("dieci", 10),
    ("undici", 11),
    ("dodici", 12),
    ("tredici", 13),
    ("quattordici", 14),
    ("quindici", 15),
    ("sedici", 16),
    ("diciassette", 17),
    ("diciotto", 18),
    ("diciannove", 19),
    ("um", 1),
    ("uma", 1),
    ("dois", 2),
    ("duas", 2),
    ("três", 3),
    ("sete", 7),
    ("oito", 8),
    ("dez", 10),
    ("doze", 12),
    ("treze", 13),
    ("catorze", 14),
    ("dezesseis", 16),
    ("dezasseis", 16),
    ("dezessete", 17),
    ("dezassete", 17),
    ("dezoito", 18),
    ("dezenove", 19),
    ("dezanove", 19),
];

/// The tens, read alone or joined to a unit word ("forty-five", "vingt et un",
/// "treinta y cinco", "trinta e cinco").
pub const TENS: &[(&str, u64)] = &[
    ("twenty", 20),
    ("thirty", 30),
    ("forty", 40),
    ("fourty", 40),
    ("fifty", 50),
    ("sixty", 60),
    ("seventy", 70),
    ("eighty", 80),
    ("ninety", 90),
    ("dix", 10),
    ("vingt", 20),
    ("trente", 30),
    ("quarante", 40),
    ("cinquante", 50),
    ("soixante", 60),
    ("quatre-vingts", 80),
    ("quatre-vingt", 80),
    ("veinte", 20),
    ("treinta", 30),
    ("cuarenta", 40),
    ("cincuenta", 50),
    ("sesenta", 60),
    ("setenta", 70),
    ("ochenta", 80),
    ("noventa", 90),
    ("venti", 20),
    ("trenta", 30),
    ("quaranta", 40),
    ("cinquanta", 50),
    ("sessanta", 60),
    ("settanta", 70),
    ("ottanta", 80),
    ("novanta", 90),
    ("vinte", 20),
    ("trinta", 30),
    ("quarenta", 40),
    ("cinquenta", 50),
    ("sessenta", 60),
    ("oitenta", 80),
];

/// The stems Italian glues a unit word to ("trent" of "trentacinque").
pub const ITALIAN_TENS: &[(&str, u64)] = &[
    ("vent", 20),
    ("trent", 30),
    ("quarant", 40),
    ("cinquant", 50),
    ("sessant", 60),
    ("settant", 70),
    ("ottant", 80),
    ("novant", 90),
];

/// The unit words Italian glues to a stem.
pub const ITALIAN_UNITS: &[(&str, u64)] = &[
    ("uno", 1),
    ("due", 2),
    ("tre", 3),
    ("tré", 3),
    ("quattro", 4),
    ("cinque", 5),
    ("sei", 6),
    ("sette", 7),
    ("otto", 8),
    ("nove", 9),
];

/// A millisecond's words.
pub const MILLISECOND: &[&str] = &[
    "milliseconds",
    "millisecond",
    "millisecondes",
    "milliseconde",
    "millisecondi",
    "millisecondo",
    "millisecs",
    "millisec",
    "millis",
    "msecs",
    "msec",
    "milisegundos",
    "milisegundo",
    "milissegundos",
    "milissegundo",
];

/// A second's words.
pub const SECOND: &[&str] = &[
    "seconds", "second", "secondes", "seconde", "secondi", "secondo", "segundos", "segundo",
    "secs", "sec", "segs", "seg",
];

/// A minute's words.
pub const MINUTE: &[&str] = &[
    "minutes", "minute", "minutos", "minuto", "minuti", "mins", "min", "mn",
];

/// An hour's words.
pub const HOUR: &[&str] = &[
    "hours", "hour", "heures", "heure", "horas", "hora", "ore", "ora", "hrs", "hr",
];

/// A day's words.
pub const DAY: &[&str] = &[
    "days",
    "day",
    "jours",
    "jour",
    "journées",
    "journée",
    "journees",
    "journee",
    "días",
    "día",
    "dias",
    "dia",
    "giorni",
    "giorno",
    "giornata",
    "gg",
];

/// A week's words.
pub const WEEK: &[&str] = &[
    "weeks",
    "week",
    "semaines",
    "semaine",
    "semanas",
    "semana",
    "settimane",
    "settimana",
    "wks",
    "wk",
];

/// A month's words.
pub const MONTH: &[&str] = &[
    "months", "month", "mois", "meses", "mes", "mês", "mesi", "mese", "mos", "mo",
];

/// A quarter's words.
pub const QUARTER: &[&str] = &[
    "quarters",
    "quarter",
    "trimestres",
    "trimestre",
    "trimestri",
];

/// A year's words.
pub const YEAR: &[&str] = &[
    "years", "year", "années", "année", "annees", "annee", "ans", "an", "años", "año", "anos",
    "ano", "anni", "anno", "yrs", "yr",
];

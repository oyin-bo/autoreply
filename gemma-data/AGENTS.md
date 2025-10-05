# Semantic Dimensions

Each word in the vocabulary is annotated with a small number of semantic dimensions (1-8) drawn from a fixed set of ~200.

When annotating, the dimensions picked for each word are those that are most pronounced, relevant and characteristic for that one word (separately from its peers).

Each dimension has a strength from -2 to +5.

Score words: 1-8 dimensions
Strength -2, -1, 0, +1, +2, +3, +4, +5
Format: `word lang:dim+score lang:dim+score`

## Dimensions

**Emotional:** happy sad angry fear surprise disgust joy sorrow rage terror shock revulsion delight despair fury anxiety awe nausea love hate pride guilt envy gratitude nostalgia hope regret shame jealousy relief sympathy contempt admiration pity resentment compassion excited calm tense relaxed cheerful gloomy enthusiastic apathetic passionate indifferent optimistic pessimistic confident insecure content dissatisfied serene agitated mild moderate intense extreme overwhelming subtle

**Social:** formal casual ceremonial intimate professional personal impersonal official informal colloquial polite rude courteous offensive respectful disrespectful tactful blunt diplomatic crude gracious boorish dominant submissive commanding obedient assertive passive authoritative subordinate controlling yielding friendly hostile welcoming rejecting inclusive exclusive open guarded trusting suspicious distant public private social solitary collective individual communal isolated

**Informational:** question statement command request declaration promise threat warning invitation offer certain uncertain definite maybe probably possibly definitely perhaps surely doubtful confident hesitant specific vague precise ambiguous exact unclear detailed general particular abstract concrete loose fact opinion belief claim proof guess assumption hypothesis theory evidence data speculation

**Communication:** direct indirect blunt subtle explicit implicit straightforward roundabout frank evasive brief verbose concise wordy terse rambling succinct lengthy literal metaphorical figurative symbolic allegorical idiomatic serious playful sarcastic ironic sincere mocking earnest teasing genuine facetious honest deceptive truthful lying authentic fake candid misleading transparent opaque

**Temporal:** past present future historical current upcoming ancient modern recent contemporary traditional futuristic brief prolonged temporary permanent fleeting lasting momentary eternal transient enduring rare frequent occasional constant sporadic regular continuous intermittent urgent patient immediate delayed pressing leisurely critical relaxed rushed unhurried beginning middle end starting ongoing finishing initial final opening closing

**Physical:** hot cold warm cool freezing boiling chilly tepid scorching icy bright dark light dim brilliant shadowy radiant murky luminous gloomy dazzling obscure large small big little huge tiny massive minuscule enormous microscopic gigantic petite heavy light weighty weightless ponderous buoyant dense airy soft hard smooth rough silky coarse velvety gritty delicate harsh fast slow quick sluggish rapid leisurely swift dawdling speedy plodding round square angular curved straight bent twisted linear circular flat

**Natural:** natural artificial organic synthetic wild domesticated raw processed primitive civilized living dead alive deceased animate inanimate vital lifeless breathing inert plant animal human machine mineral vegetable water fire earth air sky ground stone wood metal glass tree flower grass mountain river ocean cloud sun moon star

**Body:** face hand body head eye mouth nose ear skin hair heart brain blood bone muscle stomach touch taste smell see hear feel walk run jump sit stand healthy sick strong weak energetic tired fit ill vigorous exhausted visual auditory tactile olfactory gustatory

**Action:** create destroy build demolish construct ruin make break produce eliminate generate annihilate move static travel stay arrive depart enter exit advance retreat approach withdraw change transform alter modify convert shift evolve adapt mutate grow shrink expand contract increase decrease develop decline mature wither connect separate join divide unite split merge detach link unlink attach

**Cognitive:** simple complex easy difficult elementary advanced basic sophisticated straightforward intricate concrete abstract tangible intangible material conceptual physical theoretical clear confused obvious obscure understandable puzzling comprehensible baffling logical illogical rational irrational reasonable absurd sensible nonsensical coherent incoherent known unknown familiar strange evident mysterious certain doubtful

**Evaluative:** positive negative good bad pleasant unpleasant favorable unfavorable excellent poor superior inferior quality trash beautiful ugly success failure victory defeat triumph loss achievement setback important trivial significant insignificant crucial minor vital unimportant normal abnormal typical unusual ordinary strange common rare expected unexpected

## Examples
```
furious eng:angry+5 eng:intense+5 eng:negative+4
whisper eng:quiet+5 eng:soft+4 eng:secret+3
butterfly eng:animal+5 eng:natural+5 eng:beautiful+4
```

## Languages
en — English
es — Spanish
sv — Swedish
fi — Finnish
hu — Hungarian
el — Greek
uk — Ukrainian
ar — Arabic
fa — Persian (Farsi)
tr — Turkish
ka — Georgian
ig — Igbo
sw — Swahili
ha — Hausa
am — Amharic
zu — Zulu
so — Somali
hi — Hindi
bn — Bengali
ta — Tamil
te — Telugu
pa — Punjabi
zh — Chinese
ja — Japanese
ko — Korean
id — Indonesian
th — Thai
vi — Vietnamese
my — Burmese (Myanmar)
qu — Quechua
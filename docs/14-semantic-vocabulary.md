# Semantic Vocabulary Generation Plan

## Ultra-Compact Annotation Format

### Input Format (file-1.txt)
```
tree
дерево
smile
усмішка
fire
огонь
water
вода
angry
злой
```

### Dimension Reference (AGENTS.md)
```
# Semantic Dimensions (512 total)

## Emotional (0-79)
happy sad angry fear surprise disgust joy love hate proud guilty envious grateful nostalgic hopeful desperate anxious content excited calm tense relaxed cheerful gloomy furious terrified shocked repulsed delighted depressed enraged worried astonished nauseated...

## Social (80-149)  
formal casual polite rude dominant submissive friendly hostile intimate professional equal hierarchical inclusive exclusive personal impersonal warm distant open guarded trusting suspicious...

## Physical (150-199)
hot cold warm cool bright dark light heavy soft hard fast slow large small tall short wide narrow deep shallow smooth rough clean dirty wet dry...

## Natural (200-249)
natural artificial organic synthetic living dead plant animal human machine water fire earth air sky ground tree flower food drink...

## Body (250-299)
face hand body head eye mouth nose ear skin hair heart brain blood bone muscle voice touch taste smell sound sight...

## Action (300-349)
move static create destroy build damage grow shrink connect separate open close start stop begin end change stay give take push pull...

## Communication (350-399)
direct indirect brief verbose literal metaphor serious playful sarcastic honest deceptive clear vague question statement command request inform argue persuade...

## Temporal (400-429)
past present future beginning ongoing ending urgent patient quick slow temporary permanent old new recent ancient modern timeless...

## Evaluative (430-479)
positive negative good bad right wrong success failure easy difficult important trivial normal strange beautiful ugly pleasant unpleasant safe dangerous...

## Cognitive (480-511)
simple complex concrete abstract clear confused logical intuitive rational emotional certain uncertain known unknown obvious obscure...

# Output Format Rules:
- 1-5 dimensions per word
- Format: word lang:dimension+score
- Score: 1-5 (strength)
- Language code: eng/es/ua/ar/zh/ja/etc
```

### Prompt Template
```
Rank each word using semantic dimensions from AGENTS.md.
Output format: word lang:dimension+score
1-5 dimensions per word, scores 1-5.

Words:
tree
дерево
smile
усмішка
fire
огонь

Output:
```

### Expected Output
```
tree eng:natural+5 eng:plant+5 eng:large+3
дерево ua:natural+5 ua:plant+5 ua:large+3
smile eng:happy+4 eng:face+5 eng:friendly+4
усмішка ua:happy+4 ua:face+5 ua:friendly+4
fire eng:hot+5 eng:bright+5 eng:dangerous+4 eng:natural+3
огонь ua:hot+5 ua:bright+5 ua:dangerous+4 ua:natural+3
```

---

## Workflow

### 1. Generate Word Lists (per language)

**Extract from frequency lists:**
```bash
# Top 8K words per language
python extract_words.py --lang en --count 8000 --output words/en.txt
python extract_words.py --lang ua --count 8000 --output words/ua.txt
# ... for all 29 languages
```

**Example words/en.txt:**
```
happy
sad
tree
fire
water
home
love
hate
...
```

### 2. Create Batches (500 words per prompt)

```bash
# Split into batches of 500
split -l 500 words/en.txt batches/en_
split -l 500 words/ua.txt batches/ua_
```

**Result:**
```
batches/en_aa  (words 1-500)
batches/en_ab  (words 501-1000)
...
```

### 3. Score with CLI Tool

**score_batch.sh:**
```bash
#!/bin/bash
BATCH=$1
AGENTS_MD="AGENTS.md"

# Read words
WORDS=$(cat "$BATCH")

# Build prompt
PROMPT="Rank each word using semantic dimensions from this reference:

$(cat $AGENTS_MD)

Words:
$WORDS

Output format: word lang:dimension+score (1-5 dimensions, scores 1-5)
Output:"

# Call LLM (gemini-cli, gh copilot, etc)
gemini-cli "$PROMPT" > "scored_${BATCH}.txt"
```

**Usage:**
```bash
# Score single batch
./score_batch.sh batches/en_aa

# Parallel processing
ls batches/en_* | xargs -P 10 -I {} ./score_batch.sh {}
```

### 4. Parse and Merge Results

**parse_scored.py:**
```python
import re

def parse_scored_file(filepath):
    """Parse scored output into structured data."""
    results = []
    with open(filepath) as f:
        for line in f:
            # tree eng:natural+5 eng:plant+5 eng:large+3
            match = re.match(r'^(\S+)\s+(.+)$', line.strip())
            if not match:
                continue
            
            word = match.group(1)
            dims_str = match.group(2)
            
            # Parse dimensions: eng:natural+5 eng:plant+5
            dims = {}
            lang_mask = 0
            for dim_match in re.finditer(r'(\w+):(\w+)\+(\d)', dims_str):
                lang = dim_match.group(1)
                dim_name = dim_match.group(2)
                score = int(dim_match.group(3))
                
                # Look up dimension ID from AGENTS.md
                dim_id = lookup_dimension_id(dim_name)
                if dim_id is not None:
                    dims[dim_id] = score
                    lang_mask |= get_lang_bit(lang)
            
            results.append({
                'word': word,
                'dims': dims,
                'lang_mask': lang_mask
            })
    
    return results
```

### 5. Compile Binary Vocabulary

**compile_vocab.py:**
```python
def compile_vocabulary(scored_files, output_path):
    """Compile all scored words into binary vocabulary."""
    
    # Load all patterns
    patterns = []
    for filepath in scored_files:
        patterns.extend(parse_scored_file(filepath))
    
    # Tokenize each word
    tokenizer = load_sentencepiece_tokenizer()
    for pattern in patterns:
        pattern['tokens'] = tokenizer.encode(pattern['word'])
    
    # Pack into binary format
    with open(output_path, 'wb') as f:
        # Write header
        write_u32(f, len(patterns))  # num_patterns
        
        # Write pattern entries
        for p in patterns:
            # Tokens
            write_u8(f, len(p['tokens']))
            for token in p['tokens']:
                write_u32(f, token)
            
            # Dimensions (3-bit packed)
            embedding = pack_3bit_embedding(p['dims'])
            f.write(embedding)  # 192 bytes
            
            # Language mask
            write_u32(f, p['lang_mask'])
    
    print(f"Compiled {len(patterns)} patterns → {output_path}")
```

---

## Cost Estimation (Revised)

```
250K words ÷ 500 words/prompt = 500 prompts

Tokens per prompt:
- AGENTS.md reference: ~2K tokens
- 500 words: ~1K tokens  
- Output: ~2K tokens (500 words × 4 lines avg)
Total: ~5K tokens per prompt

500 prompts × 5K tokens = 2.5M tokens

Gemini Flash: $0.075/1M input + $0.30/1M output
= (1.5M × $0.075) + (1M × $0.30)
= $0.11 + $0.30 = $0.41 total!

Even with GPT-4o-mini: ~$1-2 total!
```

**Massive cost savings** with compact format!

---

## File Structure

```
semantic-vocab/
├── AGENTS.md              # Dimension reference (512 dims)
├── words/                 # Word lists per language
│   ├── en.txt            # 8K English words
│   ├── ua.txt            # 8K Ukrainian words
│   └── ...
├── batches/               # 500-word batches
│   ├── en_aa
│   ├── en_ab
│   └── ...
├── scored/                # LLM output
│   ├── scored_en_aa.txt
│   └── ...
├── scripts/
│   ├── extract_words.py
│   ├── score_batch.sh
│   ├── parse_scored.py
│   └── compile_vocab.py
└── vocab.bin              # Final binary (12 MB)
```

---

## Next Steps

1. **Create AGENTS.md** with 512 dimension taxonomy
2. **Generate word lists** (frequency extraction)
3. **Test format** with 50-word sample
4. **Validate output** parsing
5. **Scale up** to full 250K vocabulary

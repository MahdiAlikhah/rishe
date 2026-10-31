# Rishe (ریشه)

Fast Persian NLP and morphology, built in Rust. One call gives you normalized
text, tokens, lemmas, sentences, ezafe markers and a formal rewrite of colloquial
writing; morphology utilities (stemming, conjugation, inflection, token
splitting, spell correction, stopwords) and static word/sentence embeddings
live in the same core.

```bash
pip install rishe
```

```python
from rishe import Pipeline

pipe = Pipeline()
doc = pipe("اون گفت فردا می‌ره کتابخونه و یه کتاب خوب می‌خونه.")

doc.tokens       # ['آن', 'گفت', 'فردا', 'می‌رود', 'کتابخانه', 'و', 'یک', 'کتاب', 'خوب', 'می‌خواند', '.']
doc.lemmas       # ['آن', 'گفت', 'فردا', 'رفت', 'کتابخانه', 'و', 'یک', 'کتاب', 'خوب', 'خواند', '.']
doc.formal_text  # 'آن گفت فردا می‌رود کتابخانه و یک کتاب خوب می‌خواند.'
```

See the [README](https://github.com/MahdiAlikhah/rishe) for the full feature
list, the [CLI](cli.md) for command-line usage, the [API](api.md) reference, and
the [benchmarks](BENCHMARKS.md) for comparisons with Hazm and Shekar.

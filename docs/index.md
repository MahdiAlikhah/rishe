# Rishe (ریشه)

Fast Persian NLP and morphology, built in Rust. One call gives you normalized
text, tokens, lemmas, sentences, ezafe markers and a formal rewrite of colloquial
writing; morphology utilities (stemming, conjugation, inflection, token
splitting, spell correction, stopwords) live in the same core.

```bash
pip install rishe
```

```python
from rishe import Pipeline

pipe = Pipeline()
doc = pipe("اون داره تو خونه کتاب میخونه، چیه؟")

doc.tokens       # ['آن', 'دارد', 'تو', 'خانه', 'کتاب', 'می‌خواند', '،', 'چیست', '؟']
doc.lemmas       # ['آن', 'داشت', 'تو', 'خانه', 'کتاب', 'خواند', '،', 'چیست', '؟']
doc.formal_text  # 'آن دارد تو خانه کتاب می‌خواند، چیست؟'
```

See the [README](https://github.com/MahdiAlikhah/rishe) for the full feature
list, the [CLI](cli.md) for command-line usage, the [API](api.md) reference, and
the [benchmarks](BENCHMARKS.md) for comparisons with Hazm and Shekar.

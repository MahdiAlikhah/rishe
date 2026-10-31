<p align="center">
  <img src="https://raw.githubusercontent.com/MahdiAlikhah/rishe/main/docs/banner.jpg" alt="ریشه؛ پردازش زبان فارسی با Rust" width="100%">
</p>

<div dir="rtl" align="right">

# ریشه (Rishe)

**پردازش زبان فارسی با هسته‌ی Rust.**

ریشه متن خام فارسی را به داده‌ی ساخت‌یافته تبدیل می‌کند: متن نرمال‌شده،
توکن‌ها، جمله‌ها، لم‌ها، نشانه‌های اضافه، ریشه و شکل‌های صرفی. یک موتور واحد
برای پایتون و Rust.

## نصب

پایتون:

```bash
pip install rishe
```

Rust:

```bash
cargo add rishe
```

## شروع سریع

```python
from rishe import Pipeline

pipe = Pipeline()
doc = pipe("اون گفت فردا می‌ره کتابخونه و یه کتاب خوب می‌خونه.")

print(doc.tokens)
# ['آن', 'گفت', 'فردا', 'می‌رود', 'کتابخانه', 'و', 'یک', 'کتاب', 'خوب', 'می‌خواند', '.']
print(doc.lemmas)
# ['آن', 'گفت', 'فردا', 'رفت', 'کتابخانه', 'و', 'یک', 'کتاب', 'خوب', 'خواند', '.']
print(doc.formal_text)
# 'آن گفت فردا می‌رود کتابخانه و یک کتاب خوب می‌خواند.'
```

در این مثال، «می‌ره»، «کتابخونه» و «یه» به شکل رسمی برگردانده می‌شوند و
لم‌ها نیز جداگانه برگردانده می‌شوند.

## امکانات

- نرمال‌سازی، توکن‌سازی واژه و جمله و لم‌سازی.
- ریشه‌یابی، صرف افعال، تصریف، جداسازی پی‌بند، تصحیح املایی و حذف ایست‌واژه.
- تشخیص اضافه و تبدیل نوشتار محاوره‌ای به رسمی.
- امبدینگ‌های استاتیک fastText/word2vec و استخراج کلیدواژه.
- بسته‌های اختیاری POS، NER و اضافه‌ی آموخته‌شده که جداگانه دانلود می‌شوند.

## بسته‌های مدل

```bash
rishe hub pull pos-compact
rishe hub pull ner-compact
rishe hub pull ezafe-compact
```

```python
from rishe import Pipeline, hub

pipe = Pipeline(
    pos_pack=hub.path_pack("pos-compact"),
    ner_pack=hub.path_pack("ner-compact"),
    ezafe_pack=hub.path_pack("ezafe-compact"),
)

for token in pipe.pos("کتاب‌های خوب را به دوستانم می‌دهم."):
    print(token.text, token.tag)

for entity in pipe.ner("سرلشگر فیروزآبادی به پادشاه عربستان گفت."):
    print(entity.text, entity.label)
```

نصب پایه به مدل نیاز ندارد؛ بسته‌های مدل فقط وقتی لازم باشند دانلود می‌شوند.

## مستندات

- [راهنمای پایتون](https://github.com/MahdiAlikhah/rishe/blob/main/README.pypi.md)
- [راهنمای Rust](https://github.com/MahdiAlikhah/rishe/blob/main/README.crates.md)
- [API راهنمای](https://github.com/MahdiAlikhah/rishe/blob/main/docs/api.md)
- [CLI راهنمای](https://github.com/MahdiAlikhah/rishe/blob/main/docs/cli.md)
- [بنچمارک‌ها](https://github.com/MahdiAlikhah/rishe/blob/main/docs/BENCHMARKS.md)
- [README انگلیسی](https://github.com/MahdiAlikhah/rishe/blob/main/README.md)

## مجوز

انتشار تحت [Apache-2.0](https://github.com/MahdiAlikhah/rishe/blob/main/LICENSE-APACHE)
یا [MIT](https://github.com/MahdiAlikhah/rishe/blob/main/LICENSE-MIT)، به انتخاب شما.
اطلاعات منابع واژگان در
[NOTICE.md](https://github.com/MahdiAlikhah/rishe/blob/main/NOTICE.md) آمده است.

ساخته‌شده توسط مهدی علی خواه.

</div>

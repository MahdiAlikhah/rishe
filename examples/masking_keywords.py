"""Mask sensitive tokens and extract keywords."""

from rishe import Pipeline

pipe = Pipeline()
text = (
    "برای اطلاعات بیشتر به https://rishe.dev یا info@rishe.dev پیام بدهید. "
    "پردازش زبان طبیعی شاخه‌ای از هوش مصنوعی است."
)

clean = pipe.mask(text, ["url", "email"])
print("masked:  ", clean)
print("rake:    ", pipe.rake(clean, n=5))
print("textrank:", pipe.textrank(clean, n=5))

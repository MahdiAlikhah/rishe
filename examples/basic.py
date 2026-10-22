"""Normalize, tokenize, lemmatize and inspect a sentence."""

from rishe import Pipeline

pipe = Pipeline()
doc = pipe("اون داره تو خونه کتاب میخونه، چیه؟")

print("normalized:", doc.normalized_text)
print("formal:    ", doc.formal_text)
print("sentences: ", doc.sentences)
print("tokens:    ", doc.tokens)
print("lemmas:    ", doc.lemmas)
print("ezafe:     ", doc.ezafe_markers)
print("stem:      ", pipe.stem("کتاب‌ها"))
print("conjugate: ", pipe.conjugate("رفت")["present"][:3])

"""Type stubs for the compiled rishe._core extension."""

from os import PathLike
from typing import Dict, List, Optional, Tuple, Union

PathVariable = Union[str, PathLike]
"""A filesystem path accepted by the constructors: a str or an os.PathLike."""

DEFAULT_MASKERS: list[str]
"""Masker names applied by mask when maskers are omitted, in order."""

class JalaliDate:
    """A date on the Jalali, solar Hijri, calendar.

    The constructor rejects dates that do not exist. Parsing and conversion
    return None instead of raising when the input is not valid.
    """
    def __init__(self, year: int, month: int, day: int) -> None:
        """Create a Jalali date, raising ValueError when it does not exist."""
        ...
    @staticmethod
    def parse(text: str) -> Optional["JalaliDate"]:
        """Parse a Jalali date, returning None when the text is not a date.

        Accepts ۱۴۰۲/۰۵/۱۲, 12-5-1402 with the day first when the year is last,
        and ۱۲ مرداد ۱۴۰۲.
        """
        ...
    @staticmethod
    def from_gregorian(
        year: int, month: int, day: int
    ) -> Optional["JalaliDate"]:
        """Convert a Gregorian date to Jalali, or None when it is invalid."""
        ...
    @property
    def year(self) -> int:
        """Year of the date."""
        ...
    @property
    def month(self) -> int:
        """Month of the date, from 1 to 12."""
        ...
    @property
    def day(self) -> int:
        """Day of the month, starting at 1."""
        ...
    @property
    def iso(self) -> Optional[str]:
        """Gregorian date as a YYYY-MM-DD string, or None when conversion fails."""
        ...
    def to_gregorian(self) -> Optional[Tuple[int, int, int]]:
        """Return the Gregorian year, month and day, or None when conversion fails."""
        ...
    def format_persian(self) -> str:
        """Format as a Persian month-name date such as ۱۲ مرداد ۱۴۰۲."""
        ...
    def format_numeric(self) -> str:
        """Format as a Persian-digit date such as ۱۴۰۲/۰۵/۱۲."""
        ...

class TaggedToken:
    """One tagged span from a model: text, tag and character offsets."""
    @property
    def text(self) -> str:
        """Text of the span."""
        ...
    @property
    def label(self) -> str:
        """Label of the span, the same value as tag."""
        ...
    @property
    def tag(self) -> str:
        """Task label of the span, such as a part-of-speech or ezafe tag."""
        ...
    @property
    def start(self) -> int:
        """Zero-based character offset where the span starts."""
        ...
    @property
    def end(self) -> int:
        """Character offset where the span ends, exclusive."""
        ...

class Entity:
    """One named entity: its text, label and character offsets."""
    @property
    def text(self) -> str:
        """Text of the entity, with words joined by single spaces."""
        ...
    @property
    def label(self) -> str:
        """Entity label, such as PER or LOC."""
        ...
    @property
    def start(self) -> int:
        """Zero-based character offset where the entity starts."""
        ...
    @property
    def end(self) -> int:
        """Character offset where the entity ends, exclusive."""
        ...

class Dependency:
    """One dependency arc: the word, its head and the relation."""
    @property
    def text(self) -> str:
        """Text of the dependent word."""
        ...
    @property
    def head(self) -> Optional[int]:
        """Zero-based index of the head token, or None for the root."""
        ...
    @property
    def label(self) -> str:
        """Relation label, the same value as relation."""
        ...
    @property
    def relation(self) -> str:
        """Dependency relation label."""
        ...
    @property
    def start(self) -> int:
        """Zero-based character offset where the word starts."""
        ...
    @property
    def end(self) -> int:
        """Character offset where the word ends, exclusive."""
        ...

class ModelPack:
    """A task pack loaded from a directory: tokenizer, labels and ONNX model."""
    def __init__(self, path: PathVariable) -> None:
        """Load a pack directory holding labels.json, tokenizer.json and model.onnx.

        Raises ValueError when the directory or one of its files is missing or
        invalid.
        """
        ...
    def tags(self, text: str) -> List[TaggedToken]:
        """Tag text into spans, one per word, with character offsets.

        Words beyond max_len are dropped.
        """
        ...
    def tags_batch(self, texts: List[str]) -> List[List[TaggedToken]]:
        """Tag many texts, one result per input."""
        ...
    def tags_from_words(self, words: List[str]) -> List[Tuple[str, str]]:
        """Tag pre-tokenized words, returning text and label pairs.

        Words beyond max_len are dropped.
        """
        ...
    def entities(self, text: str) -> List[Entity]:
        """Return named entities by merging the token tags of text."""
        ...
    def entities_batch(self, texts: List[str]) -> List[List[Entity]]:
        """Return named entities for many texts, one result per input."""
        ...
    def labels(self) -> List[str]:
        """Return the label names the model can emit."""
        ...
    @property
    def task(self) -> str:
        """Task the pack was trained for, such as pos, ner, ezafe or depparse."""
        ...
    @property
    def max_len(self) -> int:
        """Maximum number of subword tokens the model accepts."""
        ...
    # deprecated in 0.11.x, removed in 1.0.0
    def tag(self, text: str) -> List[Tuple[str, str]]:
        """Deprecated alias of tags; emits DeprecationWarning."""
        ...
    def tag_tokens(self, text: str) -> List[TaggedToken]:
        """Deprecated alias of tags; emits DeprecationWarning."""
        ...
    def tag_batch(self, texts: List[str]) -> List[List[TaggedToken]]:
        """Deprecated alias of tags_batch; emits DeprecationWarning."""
        ...
    def tag_words(self, words: List[str]) -> List[Tuple[str, str]]:
        """Deprecated alias of tags_from_words; emits DeprecationWarning."""
        ...

class Embeddings:
    """Static word embeddings loaded from a .vec or .rvec file."""
    def __init__(self, path: PathVariable) -> None:
        """Load a word2vec or fastText .vec file or a compact .rvec file.

        Raises OSError when the file cannot be read or parsed.
        """
        ...
    def __len__(self) -> int:
        """Return the vocabulary size."""
        ...
    @property
    def dim(self) -> int:
        """Vector dimension."""
        ...
    @property
    def dtype(self) -> str:
        """Storage width of the file the vectors were loaded from: f32 or f16."""
        ...
    def contains(self, word: str) -> bool:
        """Return whether the word is in the vocabulary."""
        ...
    def vector(self, word: str) -> Optional[List[float]]:
        """Return the vector of the word, or None when it is out of vocabulary."""
        ...
    def similarity(self, first: str, second: str) -> Optional[float]:
        """Return cosine similarity of two words, or None when either is missing."""
        ...
    def sentence_vector(self, text: str) -> List[float]:
        """Return the mean of the in-vocabulary token vectors.

        Out-of-vocabulary tokens are skipped; a text with none gives a zero
        vector.
        """
        ...
    def sentence_vectors(self, texts: List[str]) -> List[List[float]]:
        """Return sentence vectors for many texts, one per input."""
        ...
    def sentence_similarity(self, first: str, second: str) -> float:
        """Return cosine similarity of two sentence vectors, 0.0 when either is empty."""
        ...
    def most_similar(self, word: str, n: int = 5) -> List[Tuple[str, float]]:
        """Return up to n most similar words with scores, best first.

        The query word itself is excluded; an out-of-vocabulary word gives an
        empty list.
        """
        ...

class ProcessedDoc:
    """The result of processing one text through the pipeline."""
    @property
    def normalized_text(self) -> str:
        """Normalized text the tokens were produced from."""
        ...
    @property
    def formal_text(self) -> str:
        """Tokens rejoined with formal spacing."""
        ...
    @property
    def sentences(self) -> List[str]:
        """Sentence strings in order; empty when the sentences stage is off."""
        ...
    @property
    def tokens(self) -> List[str]:
        """Word tokens after normalization and formalization."""
        ...
    @property
    def lemmas(self) -> List[str]:
        """Lemma of each token, in the same order and length as tokens."""
        ...
    @property
    def ezafe_markers(self) -> List[bool]:
        """For each token, whether an ezafe is predicted before the next token.

        The list has the same length as tokens; the last entry is always false.
        """
        ...

class Pipeline:
    """The Persian NLP pipeline: normalization, tokenization, morphology and models."""
    def __init__(
        self,
        unify_digits: bool = True,
        strip_diacritics: bool = True,
        fix_zwnj: bool = True,
        pos_pack: Optional[PathVariable] = None,
        ner_pack: Optional[PathVariable] = None,
        ezafe_pack: Optional[PathVariable] = None,
        depparse_pack: Optional[PathVariable] = None,
    ) -> None:
        """Create a pipeline.

        The boolean options configure the normalizer. Each pack path loads a
        model for that task; a pack of the wrong task raises ValueError.
        """
        ...
    def __call__(self, text: str) -> ProcessedDoc:
        """Process text with every stage, the same as process."""
        ...
    def process(
        self, text: str, stages: Optional[List[str]] = None
    ) -> ProcessedDoc:
        """Run the pipeline and return a ProcessedDoc.

        stages selects and orders the normalize, formalize, lemmatize, sentences
        and ezafe stages; omit it or pass an empty list for all of them. An
        unknown name raises ValueError.
        """
        ...
    def process_batch(self, texts: List[str]) -> List[ProcessedDoc]:
        """Process many texts, returning one ProcessedDoc per input."""
        ...
    def pos(self, text: str) -> List[TaggedToken]:
        """Return part-of-speech tagged tokens.

        Raises ValueError when no pos pack is loaded.
        """
        ...
    def ner(self, text: str) -> List[Entity]:
        """Return named entities.

        Raises ValueError when no ner pack is loaded.
        """
        ...
    def ezafe(self, text: str) -> List[TaggedToken]:
        """Return tokens tagged with ezafe markers.

        Raises ValueError when no ezafe pack is loaded.
        """
        ...
    def pos_batch(self, texts: List[str]) -> List[List[TaggedToken]]:
        """Return part-of-speech tags for many texts, one result per input."""
        ...
    def ner_batch(self, texts: List[str]) -> List[List[Entity]]:
        """Return named entities for many texts, one result per input."""
        ...
    def ezafe_batch(self, texts: List[str]) -> List[List[TaggedToken]]:
        """Return ezafe-tagged tokens for many texts, one result per input."""
        ...
    def depparse(self, text: str) -> List[Dependency]:
        """Return dependency arcs.

        Raises ValueError when no depparse pack is loaded.
        """
        ...
    def depparse_batch(self, texts: List[str]) -> List[List[Dependency]]:
        """Return dependency arcs for many texts, one result per input."""
        ...
    def normalize(self, text: str, stages: Optional[List[str]] = None) -> str:
        """Normalize text, optionally with an ordered list of operator names.

        Omit stages or pass an empty list for the full default normalizer; an
        unknown name raises ValueError.
        """
        ...
    def tokenize(
        self,
        text: str,
        join_verb_parts: bool = False,
        join_abbreviations: bool = False,
        separate_emoji: bool = False,
        replace_links: bool = False,
        replace_ids: bool = False,
        replace_emails: bool = False,
        replace_numbers: bool = False,
        replace_hashtags: bool = False,
    ) -> List[str]:
        """Tokenize into surface words without normalizing.

        Call normalize first or use process for normalized tokens. The options
        enable verb-part joining, abbreviation joining, emoji separation and
        replacement of links, ids, emails, numbers and hashtags.
        """
        ...
    def tokenize_spans(self, text: str) -> List[Tuple[str, int, int]]:
        """Return token, start and end triples with zero-based character offsets.

        The end offset is exclusive.
        """
        ...
    def sent_tokenize(self, text: str) -> List[str]:
        """Split text into sentences."""
        ...
    def mask(
        self, text: str, maskers: List[str], mask_token: Optional[str] = None
    ) -> str:
        """Mask or remove the parts of text selected by maskers.

        maskers is applied in order; an empty list returns the text unchanged
        and an unknown name raises ValueError. mask_token overrides the default
        replacement of every masker.
        """
        ...
    def number_to_words(self, value: Union[int, str]) -> str:
        """Return the Persian words for an int or a digit string.

        Raises ValueError for a string that is not a number and TypeError for
        other types.
        """
        ...
    def ngrams(self, text: str, min_n: int = 1, max_n: int = 1) -> List[str]:
        """Return every n-gram from min_n to max_n, grouped by n in text order.

        Raises ValueError when min_n is zero or max_n is smaller than min_n.
        """
        ...
    def persianize(self, text: str) -> str:
        """Replace Persian loanwords with their first Persian alternative."""
        ...
    def to_tajik(self, text: str) -> str:
        """Transliterate Persian text to Tajik Cyrillic."""
        ...
    def to_persian(self, text: str) -> str:
        """Transliterate Tajik Cyrillic text to Persian."""
        ...
    def persianize_suggest(
        self, text: str
    ) -> List[Tuple[str, List[str], int, int]]:
        """Return loanword matches as word, alternatives, start and end.

        Offsets are zero-based character offsets; the end offset is exclusive.
        """
        ...
    def rake(self, text: str, n: int = 5, max_length: int = 3) -> List[str]:
        """Return up to n RAKE keywords; max_length caps the words in a phrase."""
        ...
    def textrank(
        self, text: str, n: int = 5, max_length: int = 3, window: int = 2
    ) -> List[str]:
        """Return up to n TextRank keywords.

        max_length caps the words in a phrase and window sets the co-occurrence
        window.
        """
        ...
    def keyboard_noise(self, text: str, seed: Optional[int] = None) -> str:
        """Return text with random keyboard typos.

        A seed makes the result reproducible; None picks a fresh seed.
        """
        ...
    def ocr_noise(self, text: str, seed: Optional[int] = None) -> str:
        """Return text with random OCR confusions.

        A seed makes the result reproducible; None picks a fresh seed.
        """
        ...
    def whitespace_noise(self, text: str, seed: Optional[int] = None) -> str:
        """Return text with random spacing and ZWNJ damage.

        A seed makes the result reproducible; None picks a fresh seed.
        """
        ...
    def lemmatize(self, word: str) -> str:
        """Return the lemma of a single word."""
        ...
    def lemmatize_verb(self, word: str) -> Tuple[str, Optional[str]]:
        """Return the lemma and its present stem, or None when it is not a verb."""
        ...
    def stem(self, word: str) -> str:
        """Return the stem of a single word."""
        ...
    def conjugate(self, verb: str) -> Optional[Dict[str, List[str]]]:
        """Return the forms of a verb keyed by tense, or None when it is unknown.

        The eight keys are past, negative_past, imperfective_past, present,
        negative_present, subjunctive, future and perfect; each list holds six
        persons, singular then plural.
        """
        ...
    def split_token(self, token: str) -> List[str]:
        """Split one token into its parts, such as a stem and a suffix."""
        ...
    def spell_suggest(self, word: str, n: int = 5) -> List[str]:
        """Return up to n spelling suggestions for a word, best first."""
        ...
    def spell_correct(self, text: str) -> str:
        """Correct misspelled words in text and return the corrected text."""
        ...
    def comparative(self, adj: str) -> str:
        """Return the comparative form of an adjective."""
        ...
    def superlative(self, adj: str) -> str:
        """Return the superlative form of an adjective."""
        ...
    def plural(self, noun: str) -> str:
        """Return the plural form of a noun."""
        ...
    def possessive(self, noun: str, person: int) -> Optional[str]:
        """Return the noun with a possessive suffix for a person index.

        The index runs from 0 to 5; an index outside that range gives None.
        """
        ...
    def is_stopword(self, word: str) -> bool:
        """Return whether the word is a stopword."""
        ...
    def remove_stopwords(self, tokens: List[str]) -> List[str]:
        """Return the tokens without stopwords, keeping order."""
        ...
    def stopwords(self) -> List[str]:
        """Return every stopword."""
        ...
    # deprecated in 0.11.x, removed in 1.0.0
    def pipe(self, texts: List[str]) -> List[ProcessedDoc]:
        """Deprecated alias of process_batch; emits DeprecationWarning."""
        ...
    def process_stages(self, text: str, stages: List[str]) -> ProcessedDoc:
        """Deprecated alias of process; emits DeprecationWarning."""
        ...
    def normalize_stages(self, text: str, stages: List[str]) -> str:
        """Deprecated alias of normalize; emits DeprecationWarning."""
        ...
    def mask_stages(
        self, text: str, stages: List[str], mask_token: Optional[str] = None
    ) -> str:
        """Deprecated alias of mask; emits DeprecationWarning."""
        ...
    def word_tokenize(self, text: str) -> List[str]:
        """Deprecated alias of tokenize that normalizes first; emits DeprecationWarning."""
        ...
    def word_tokenize_with(
        self,
        text: str,
        join_verb_parts: bool = False,
        join_abbreviations: bool = False,
        separate_emoji: bool = False,
        replace_links: bool = False,
        replace_ids: bool = False,
        replace_emails: bool = False,
        replace_numbers: bool = False,
        replace_hashtags: bool = False,
    ) -> List[str]:
        """Deprecated alias of tokenize that normalizes first; emits DeprecationWarning."""
        ...
    def lemma_full(self, word: str) -> Tuple[str, Optional[str]]:
        """Deprecated alias of lemmatize_verb; emits DeprecationWarning."""
        ...

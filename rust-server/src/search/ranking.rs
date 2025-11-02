//! Ranking & Scoring System
//!
//! Implements multi-signal scoring for search results with configurable weights.

use super::fuzzy::{FuzzyMatch, MatchType};

/// Scoring weights for different match signals
#[derive(Debug, Clone)]
pub struct ScoringWeights {
    /// Base weight for fuzzy match score
    pub fuzzy_base: f64,
    /// Position multiplier based on match type
    pub position_multipliers: PositionMultipliers,
    /// Maximum proximity boost
    pub max_proximity_boost: f64,
    /// Bonus multiplier for exact matches (especially quoted text)
    pub exact_match_bonus: f64,
    /// Bonus for exact Unicode match (vs normalized)
    pub unicode_exact_bonus: f64,
}

/// Position-based multipliers for different match types
#[derive(Debug, Clone)]
pub struct PositionMultipliers {
    /// Full word match
    pub full_word: f64,
    /// Beginning of word
    pub word_start: f64,
    /// End of word
    pub word_end: f64,
    /// Middle of word
    pub word_middle: f64,
    /// Multi-word match
    pub multi_word: f64,
}

impl Default for ScoringWeights {
    fn default() -> Self {
        Self {
            fuzzy_base: 1.0,
            position_multipliers: PositionMultipliers::default(),
            max_proximity_boost: 1.0,
            exact_match_bonus: 10.0,
            unicode_exact_bonus: 1.2,
        }
    }
}

impl Default for PositionMultipliers {
    fn default() -> Self {
        Self {
            full_word: 1.0,      // Highest priority
            word_start: 0.8,     // Second highest
            word_end: 0.6,       // Third
            word_middle: 0.4,    // Lowest
            multi_word: 0.9,     // High, but slightly less than full word
        }
    }
}

/// Complete match score with all components
#[derive(Debug, Clone)]
pub struct MatchScore {
    /// Base fuzzy match score
    pub base_score: f64,
    /// Position-based weight
    pub position_weight: f64,
    /// Proximity boost
    pub proximity_boost: f64,
    /// Whether this is an exact match
    pub is_exact_match: bool,
    /// Whether this is an exact Unicode match
    pub is_exact_unicode: bool,
    /// Final weighted score
    pub final_score: f64,
    /// Match type
    pub match_type: MatchType,
}

impl MatchScore {
    /// Calculate final score from fuzzy match and additional signals
    pub fn calculate(
        fuzzy_match: &FuzzyMatch,
        proximity_score: f64,
        is_exact: bool,
        is_exact_unicode: bool,
        weights: &ScoringWeights,
    ) -> Self {
        let base_score = fuzzy_match.score as f64 * weights.fuzzy_base;
        
        // Get position multiplier based on match type
        let position_weight = match fuzzy_match.match_type {
            MatchType::FullWord => weights.position_multipliers.full_word,
            MatchType::WordStart => weights.position_multipliers.word_start,
            MatchType::WordEnd => weights.position_multipliers.word_end,
            MatchType::WordMiddle => weights.position_multipliers.word_middle,
            MatchType::MultiWord => weights.position_multipliers.multi_word,
        };
        
        // Calculate proximity boost (scaled to max_proximity_boost)
        let proximity_boost = proximity_score * weights.max_proximity_boost;
        
        // Start with base score and apply position weight
        let mut final_score = base_score * position_weight;
        
        // Add proximity boost
        final_score += proximity_boost * base_score;
        
        // Apply exact match bonus (multiplicative)
        if is_exact {
            final_score *= weights.exact_match_bonus;
        }
        
        // Apply Unicode exact bonus
        if is_exact_unicode {
            final_score *= weights.unicode_exact_bonus;
        }
        
        Self {
            base_score,
            position_weight,
            proximity_boost,
            is_exact_match: is_exact,
            is_exact_unicode,
            final_score,
            match_type: fuzzy_match.match_type,
        }
    }
    
    /// Create a score for exact match (highest possible)
    pub fn exact_match(_haystack_len: usize, weights: &ScoringWeights) -> Self {
        let base_score = 1000.0; // High base score for exact match
        
        Self {
            base_score,
            position_weight: weights.position_multipliers.full_word,
            proximity_boost: weights.max_proximity_boost,
            is_exact_match: true,
            is_exact_unicode: true,
            final_score: base_score 
                * weights.position_multipliers.full_word 
                * weights.exact_match_bonus
                * weights.unicode_exact_bonus,
            match_type: MatchType::FullWord,
        }
    }
}

/// Normalize scores to 0-1 range for comparison across different sources
pub fn normalize_scores(scores: &mut [MatchScore]) {
    if scores.is_empty() {
        return;
    }
    
    // Find min and max scores
    let max_score = scores
        .iter()
        .map(|s| s.final_score)
        .max_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap_or(1.0);
    
    let min_score = scores
        .iter()
        .map(|s| s.final_score)
        .min_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap_or(0.0);
    
    let range = max_score - min_score;
    
    if range > 0.0 {
        for score in scores.iter_mut() {
            score.final_score = (score.final_score - min_score) / range;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_weights() {
        let weights = ScoringWeights::default();
        assert_eq!(weights.fuzzy_base, 1.0);
        assert_eq!(weights.exact_match_bonus, 10.0);
    }

    #[test]
    fn test_position_multipliers() {
        let multipliers = PositionMultipliers::default();
        assert!(multipliers.full_word > multipliers.word_start);
        assert!(multipliers.word_start > multipliers.word_end);
        assert!(multipliers.word_end > multipliers.word_middle);
    }

    #[test]
    fn test_exact_match_score() {
        let weights = ScoringWeights::default();
        let score = MatchScore::exact_match(10, &weights);
        
        assert!(score.is_exact_match);
        assert!(score.is_exact_unicode);
        assert!(score.final_score > 1000.0); // Should be very high
    }

    #[test]
    fn test_normalize_scores() {
        let mut scores = vec![
            MatchScore {
                base_score: 100.0,
                position_weight: 1.0,
                proximity_boost: 0.5,
                is_exact_match: false,
                is_exact_unicode: false,
                final_score: 100.0,
                match_type: MatchType::FullWord,
            },
            MatchScore {
                base_score: 50.0,
                position_weight: 0.8,
                proximity_boost: 0.3,
                is_exact_match: false,
                is_exact_unicode: false,
                final_score: 50.0,
                match_type: MatchType::WordStart,
            },
            MatchScore {
                base_score: 200.0,
                position_weight: 1.0,
                proximity_boost: 0.7,
                is_exact_match: false,
                is_exact_unicode: false,
                final_score: 200.0,
                match_type: MatchType::FullWord,
            },
        ];
        
        normalize_scores(&mut scores);
        
        // Check all scores are in 0-1 range
        for score in &scores {
            assert!(score.final_score >= 0.0);
            assert!(score.final_score <= 1.0);
        }
        
        // Max should be 1.0, min should be 0.0
        let max = scores.iter().map(|s| s.final_score).max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap();
        let min = scores.iter().map(|s| s.final_score).min_by(|a, b| a.partial_cmp(b).unwrap()).unwrap();
        
        assert!((max - 1.0).abs() < 0.001);
        assert!((min - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_normalize_empty() {
        let mut scores: Vec<MatchScore> = vec![];
        normalize_scores(&mut scores); // Should not panic
        assert!(scores.is_empty());
    }
}

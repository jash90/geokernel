use super::Work;

#[derive(Clone, Copy)]
pub(super) struct Field(pub(super) i128);

fn trim(polynomial: &mut Vec<i128>) {
    while polynomial.last() == Some(&0) {
        polynomial.pop();
    }
}

impl Field {
    pub(super) fn inverse(self, value: i128) -> i128 {
        let mut power = self.0 - 2;
        let mut base = value;
        let mut inverse = 1;
        while power != 0 {
            if power % 2 != 0 {
                inverse = inverse * base % self.0;
            }
            base = base * base % self.0;
            power /= 2;
        }
        inverse
    }

    fn subtract(self, a: &[i128], b: &[i128]) -> Vec<i128> {
        let mut result = a.to_vec();
        result.resize(a.len().max(b.len()), 0);
        for (term, &other) in result.iter_mut().zip(b) {
            *term = (*term - other).rem_euclid(self.0);
        }
        trim(&mut result);
        result
    }

    pub(super) fn multiply(
        self,
        left: &[i128],
        right: &[i128],
        work: &mut Work<'_>,
    ) -> Option<Vec<i128>> {
        work.spend(left.len() * right.len())?;
        if left.is_empty() || right.is_empty() {
            return Some(vec![]);
        }
        let mut result = vec![0; left.len() + right.len() - 1];
        for (i, &a) in left.iter().enumerate() {
            for (term, &b) in result[i..].iter_mut().zip(right) {
                *term = (*term + a * b) % self.0;
            }
        }
        trim(&mut result);
        Some(result)
    }

    pub(super) fn divide(
        self,
        mut dividend: Vec<i128>,
        divisor: &[i128],
        work: &mut Work<'_>,
    ) -> Option<(Vec<i128>, Vec<i128>)> {
        let inverse = self.inverse(*divisor.last()?);
        if dividend.len() < divisor.len() {
            return Some((vec![], dividend));
        }
        let steps = dividend.len() - divisor.len() + 1;
        work.spend(steps * divisor.len())?;
        let mut quotient = vec![0; steps];
        for shift in (0..steps).rev() {
            let end = shift + divisor.len() - 1;
            let factor = dividend[end] * inverse % self.0;
            for (value, &term) in dividend[shift..=end].iter_mut().zip(divisor) {
                *value = (*value - factor * term).rem_euclid(self.0);
            }
            quotient[shift] = factor;
        }
        trim(&mut dividend);
        trim(&mut quotient);
        Some((quotient, dividend))
    }

    pub(super) fn gcd(
        self,
        mut left: Vec<i128>,
        mut right: Vec<i128>,
        work: &mut Work<'_>,
    ) -> Option<Vec<i128>> {
        trim(&mut right);
        while !right.is_empty() {
            let (_, remainder) = self.divide(left, &right, work)?;
            left = right;
            right = remainder;
        }
        if let Some(&lead) = left.last() {
            let inverse = self.inverse(lead);
            for term in &mut left {
                *term = *term * inverse % self.0;
            }
        }
        Some(left)
    }

    pub(super) fn polynomial_inverse(
        self,
        polynomial: &[i128],
        modulus: &[i128],
        work: &mut Work<'_>,
    ) -> Option<Vec<i128>> {
        let (_, mut right) = self.divide(polynomial.to_vec(), modulus, work)?;
        let mut left = modulus.to_vec();
        let mut a = Vec::new();
        let mut b = vec![1];
        while !right.is_empty() {
            let (quotient, remainder) = self.divide(left, &right, work)?;
            left = right;
            right = remainder;
            let product = self.multiply(&quotient, &b, work)?;
            (a, b) = (b, self.subtract(&a, &product));
        }
        let &[constant] = left.as_slice() else {
            return None;
        };
        let inverse = self.inverse(constant);
        for value in &mut a {
            *value = *value * inverse % self.0;
        }
        Some(a)
    }

    fn power_of_x(
        self,
        mut power: i128,
        modulus: &[i128],
        work: &mut Work<'_>,
    ) -> Option<Vec<i128>> {
        let mut base = vec![0, 1];
        let mut result = vec![1];
        while power != 0 {
            if power % 2 != 0 {
                let product = self.multiply(&base, &result, work)?;
                result = self.divide(product, modulus, work)?.1;
            }
            let square = self.multiply(&base, &base, work)?;
            base = self.divide(square, modulus, work)?.1;
            power /= 2;
        }
        Some(result)
    }

    fn fixed_space(self, polynomial: &[i128], work: &mut Work<'_>) -> Option<Vec<Vec<i128>>> {
        let degree = polynomial.len() - 1;
        work.spend(degree * degree * degree)?;
        let mut matrix = vec![vec![0_i128; degree]; degree];
        let xp = self.power_of_x(self.0, polynomial, work)?;
        let mut power = vec![1];
        for column in 0..degree {
            for (row, &coefficient) in matrix.iter_mut().zip(&power) {
                row[column] = coefficient;
            }
            matrix[column][column] = (matrix[column][column] - 1).rem_euclid(self.0);
            let product = self.multiply(&power, &xp, work)?;
            power = self.divide(product, polynomial, work)?.1;
        }
        let mut pivot_columns = Vec::new();
        let mut free_columns = Vec::new();
        for column in 0..degree {
            let row = pivot_columns.len();
            let Some(pivot) = (row..degree).find(|&i| matrix[i][column] != 0) else {
                free_columns.push(column);
                continue;
            };
            matrix.swap(row, pivot);
            let inverse = self.inverse(matrix[row][column]);
            for value in &mut matrix[row][column..] {
                *value = *value * inverse % self.0;
            }
            let normalized = matrix[row].clone();
            for (i, other) in matrix.iter_mut().enumerate() {
                if i != row {
                    let weight = other[column];
                    for (entry, &pivot_entry) in
                        other[column..].iter_mut().zip(&normalized[column..])
                    {
                        *entry = (*entry - weight * pivot_entry).rem_euclid(self.0);
                    }
                }
            }
            pivot_columns.push(column);
        }
        Some(
            free_columns
                .into_iter()
                .map(|column| {
                    let mut vector = vec![0; degree];
                    vector[column] = 1;
                    for (row, &index) in pivot_columns.iter().enumerate() {
                        vector[index] = (-matrix[row][column]).rem_euclid(self.0);
                    }
                    vector
                })
                .collect(),
        )
    }

    /// Berlekamp's fixed-space algorithm for a monic square-free polynomial.
    /// https://doi.org/10.1002/j.1538-7305.1967.tb03174.x
    pub(super) fn irreducible_factors(
        self,
        polynomial: Vec<i128>,
        work: &mut Work<'_>,
    ) -> Option<Vec<Vec<i128>>> {
        let basis = self.fixed_space(&polynomial, work)?;
        let mut factors = vec![polynomial];
        let count = basis.len();
        for mut vector in basis {
            trim(&mut vector);
            if vector.len() <= 1 {
                continue;
            }
            let constant = vector[0];
            for value in 0..self.0 {
                vector[0] = (constant - value).rem_euclid(self.0);
                let mut index = 0;
                while index < factors.len() {
                    if factors[index].len() > 2 {
                        let factor = self.gcd(factors[index].clone(), vector.clone(), work)?;
                        if factor.len() > 1 && factor.len() < factors[index].len() {
                            let (quotient, rest) =
                                self.divide(std::mem::take(&mut factors[index]), &factor, work)?;
                            debug_assert!(rest.is_empty());
                            factors[index] = quotient;
                            factors.push(factor);
                        }
                    }
                    index += 1;
                }
                if factors.len() == count {
                    return Some(factors);
                }
            }
        }
        (factors.len() == count).then_some(factors)
    }
}

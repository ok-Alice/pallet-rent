pub fn convert_to_primitive<F, T>(value: F) -> Result<T, ()>
where
	F: TryInto<T>,
{
	Ok(TryInto::<T>::try_into(value).ok().unwrap())
}

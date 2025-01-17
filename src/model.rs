use crate::error::{CatBoostError, CatBoostResult};
use catboost_sys;
use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

pub struct Model {
    handle: *mut catboost_sys::ModelCalcerHandle,
}

impl Model {
    fn new() -> Self {
        let model_handle = unsafe { catboost_sys::ModelCalcerCreate() };
        Model {
            handle: model_handle,
        }
    }

    /// Load a model from a file
    pub fn load<P: AsRef<Path>>(path: P) -> CatBoostResult<Self> {
        let model = Model::new();
        let path_c_str = CString::new(path.as_ref().as_os_str().as_bytes()).unwrap();
        CatBoostError::check_return_value(unsafe {
            catboost_sys::LoadFullModelFromFile(model.handle, path_c_str.as_ptr())
        })?;
        Ok(model)
    }

    /// Load a model from a buffer
    pub fn load_buffer<P: AsRef<Vec<u8>>>(buffer: P) -> CatBoostResult<Self> {
        let model = Model::new();
        CatBoostError::check_return_value(unsafe {
            catboost_sys::LoadFullModelFromBuffer(
                model.handle,
                buffer.as_ref().as_ptr() as *const std::os::raw::c_void,
                buffer.as_ref().len(),
            )
        })?;
        Ok(model)
    }

    /// Calculate raw model predictions on float features and string categorical feature values
    pub fn calc_model_prediction(
        &self,
        float_features: Vec<Vec<f32>>,
        cat_features: Vec<Vec<String>>,
    ) -> CatBoostResult<Vec<f64>> {
        let mut float_features_ptr = float_features
            .iter()
            .map(|x| x.as_ptr())
            .collect::<Vec<_>>();

        let hashed_cat_features = cat_features
            .iter()
            .map(|doc_cat_features| {
                doc_cat_features
                    .iter()
                    .map(|cat_feature| unsafe {
                        catboost_sys::GetStringCatFeatureHash(
                            cat_feature.as_ptr() as *const std::os::raw::c_char,
                            cat_feature.len(),
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        let mut hashed_cat_features_ptr = hashed_cat_features
            .iter()
            .map(|x| x.as_ptr())
            .collect::<Vec<_>>();

        let mut prediction = vec![0.0; float_features.len()];
        CatBoostError::check_return_value(unsafe {
            catboost_sys::CalcModelPredictionWithHashedCatFeatures(
                self.handle,
                float_features.len(),
                float_features_ptr.as_mut_ptr(),
                float_features[0].len(),
                hashed_cat_features_ptr.as_mut_ptr(),
                cat_features[0].len(),
                prediction.as_mut_ptr(),
                prediction.len(),
            )
        })?;
        Ok(prediction)
    }

    /// Apply sigmoid to get predict probability
    // https://catboost.ai/en/docs/concepts/output-data_model-value-output#classification
    pub fn calc_predict_proba(
        &self,
        float_features: Vec<Vec<f32>>,
        cat_features: Vec<Vec<String>>,
    ) -> CatBoostResult<Vec<f64>> {
        let raw_results = self.calc_model_prediction(float_features, cat_features)?;
        let probabilities = raw_results.into_iter().map(sigmoid).collect();
        Ok(probabilities)
    }

    /// Get expected float feature count for model
    pub fn get_float_features_count(&self) -> usize {
        unsafe { catboost_sys::GetFloatFeaturesCount(self.handle) }
    }

    /// Get expected categorical feature count for model
    pub fn get_cat_features_count(&self) -> usize {
        unsafe { catboost_sys::GetCatFeaturesCount(self.handle) }
    }

    /// Get number of trees in model
    pub fn get_tree_count(&self) -> usize {
        unsafe { catboost_sys::GetTreeCount(self.handle) }
    }

    /// Get number of dimensions in model
    pub fn get_dimensions_count(&self) -> usize {
        unsafe { catboost_sys::GetDimensionsCount(self.handle) }
    }
}

impl Drop for Model {
    fn drop(&mut self) {
        unsafe { catboost_sys::ModelCalcerDelete(self.handle) };
    }
}

// Should be thread safe as stated here: https://github.com/catboost/catboost/issues/272
unsafe impl Send for Model {}

unsafe impl Sync for Model {}

fn sigmoid(x: f64) -> f64 {
    1. / (1. + (-x).exp())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_model() {
        let model = Model::load("files/model.bin");
        assert!(model.is_ok());
    }

    #[test]
    fn load_model_buffer() {
        let buffer: Vec<u8> = read_fast("files/model.bin").unwrap();
        let model = Model::load_buffer(buffer);
        assert!(model.is_ok());
    }

    #[test]
    fn calc_prediction() {
        let model = Model::load("files/model.bin").unwrap();
        let prediction = model
            .calc_model_prediction(
                vec![
                    vec![-10.0, 5.0, 753.0],
                    vec![30.0, 1.0, 760.0],
                    vec![40.0, 0.1, 705.0],
                ],
                vec![
                    vec![String::from("north")],
                    vec![String::from("south")],
                    vec![String::from("south")],
                ],
            )
            .unwrap();

        assert_eq!(prediction[0], 0.9980003729960197);
        assert_eq!(prediction[1], 0.00249414628534181);
        assert_eq!(prediction[2], -0.0013677527881450977);
    }

    #[test]
    fn get_model_stats() {
        let model = Model::load("files/model.bin").unwrap();

        assert_eq!(model.get_cat_features_count(), 1);
        assert_eq!(model.get_float_features_count(), 3);
        assert_eq!(model.get_tree_count(), 1000);
        assert_eq!(model.get_dimensions_count(), 1);
    }

    use std::io::Read;
    fn read_fast<P: AsRef<std::path::Path>>(path: P) -> std::io::Result<Vec<u8>> {
        let mut file = std::fs::File::open(path)?;
        let meta = file.metadata()?;
        let size = meta.len() as usize;
        let mut data = Vec::with_capacity(size);
        data.resize(size, 0);
        file.read_exact(&mut data)?;
        Ok(data)
    }
}
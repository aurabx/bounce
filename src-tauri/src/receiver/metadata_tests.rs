#[cfg(test)]
mod tests {
    use crate::receiver::metadata::Metadata;
    use dicom::core::{DataElement, VR};
    use dicom::dicom_value;
    use dicom::dictionary_std::tags;
    use dicom::object::InMemDicomObject;

    #[test]
    fn test_create_study_info_from_dicom() {
        let study_uid = "1.2.840.10008.1.2.3.4.5";
        let series_uid = "1.2.840.10008.1.2.3.4.5.1";
        
        // Create a DICOM object with some tags
        let obj = InMemDicomObject::from_element_iter([
            DataElement::new(
                tags::STUDY_INSTANCE_UID,
                VR::UI,
                dicom_value!(Str, study_uid),
            ),
            DataElement::new(
                tags::SERIES_INSTANCE_UID,
                VR::UI,
                dicom_value!(Str, series_uid),
            ),
            DataElement::new(
                tags::PATIENT_NAME,
                VR::PN,
                dicom_value!(Str, "Doe^John"),
            ),
            DataElement::new(
                tags::MODALITY,
                VR::CS,
                dicom_value!(Str, "CT"),
            ),
            DataElement::new(
                tags::STUDY_DESCRIPTION,
                VR::LO,
                dicom_value!(Str, "Brain CT"),
            ),
        ]);

        let result = Metadata::create_new_study_info(&obj, study_uid);
        
        assert!(result.is_ok());
        let study_info = result.unwrap();
        
        assert_eq!(study_info.study_uid, study_uid);
        assert_eq!(study_info.patient_name, Some("Doe^John".to_string()));
        assert_eq!(study_info.study_description, Some("Brain CT".to_string()));
        
        // Note: Series are not populated by create_new_study_info, they are added later in update_study_metadata_json
        // but create_new_study_info initializes the map
        assert!(study_info.series.is_empty());
    }
    
    #[test]
    fn test_create_study_info_missing_optional_tags() {
        let study_uid = "1.2.840.10008.1.2.3.4.5";
        
        // Minimal DICOM object
        let obj = InMemDicomObject::from_element_iter([
            DataElement::new(
                tags::STUDY_INSTANCE_UID,
                VR::UI,
                dicom_value!(Str, study_uid),
            ),
        ]);

        let result = Metadata::create_new_study_info(&obj, study_uid);
        
        assert!(result.is_ok());
        let study_info = result.unwrap();
        
        assert_eq!(study_info.study_uid, study_uid);
        assert!(study_info.patient_name.is_none());
        assert!(study_info.study_description.is_none());
    }
}

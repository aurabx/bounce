import TextInput from "@/app/components/Fields/TextInput";
import SelectInput from "@/app/components/Fields/SelectInput";

const fields = [{
    config: {
        label: 'Region',
        key: 'region',
        help: 'Select the region you login to when connecting to Aurabox',
        options: {
            au: "Australia",
            uk: "United Kingdom",
            sg: "Singapore",
        }
    },
    component: SelectInput
},{
    config: {
        label: 'Api Key',
        key: 'api_key',
        help: 'You can generate an api key in the account section of Aurabox',
    },
    component: TextInput
},{
    config: {
        label: 'Port',
        key: 'port',
        help: 'Port where this is going to run',
    },
    component: TextInput
},{
    config: {
        label: 'Storage directory',
        key: 'base_dir',
        help: 'Local storage directory for bounce to use',
        suffix_button: 'Select storage directory',
        readOnly: true,
    },
    component: TextInput
},{
    config: {
        label: 'Mode',
        key: 'mode',
        help: 'Select the mode for bounce to run in',
        options: {
            production: "Production",
            staging: "Staging",
            development: "Development",
            local: "Local",
        }
    },
    component: SelectInput
}];



export default fields;
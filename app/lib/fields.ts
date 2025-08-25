import TextInput from "@/app/components/Fields/TextInput";
import SelectInput from "@/app/components/Fields/SelectInput";


export const fieldKeys = [
    'api_key',
    'port',
    'base_dir',
    'ae_title',
    'ip_address',
    'setup_complete',
]

export const fields = [{
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
        label: 'AE Title',
        key: 'ae_title',
        help: 'AE Title used for the DICOM server. Will default to: BOUNCE',
    },
    component: TextInput
},{
    config: {
        label: 'IP Address',
        key: 'ip_address',
        help: 'Bind ip address for DICOM server, leave blank to bind to all interfaces',
    },
    component: TextInput
},{
    config: {
        label: 'Storage directory',
        key: 'base_dir',
        help: 'Local storage directory for bounce to use',
        suffix_button: 'Select a storage directory',
        readOnly: true,
    },
    component: TextInput
}];


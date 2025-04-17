import TextInput from "@/app/components/Fields/TextInput";
import SelectInput from "@/app/components/Fields/SelectInput";


export const fieldKeys = [
    'api_key',
    'port',
    'base_dir',
    'setup_complete'
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
        label: 'Storage directory',
        key: 'base_dir',
        help: 'Local storage directory for bounce to use',
        suffix_button: 'Select storage directory',
        readOnly: true,
    },
    component: TextInput
}];

